use actix_web::{web::{self, Data, Json}, HttpRequest, Responder, HttpResponse, cookie::{time::Duration, Cookie}};
use serde::{Serialize, Deserialize};
use sqlx::{self, Row};

use chrono::{self, NaiveDateTime};
use log;

use crate::models::{
    ServerResponse, User, UserCredentials, CreateUserBody, ChangePasswordBody, 
    ChangeForgottenPasswordBody, ChangeEmailBody, ChangeUsernameBody, StoredUser
};
use crate::{PostgresDB, RedisDB, APP_SCHEMA, USERS_TABLE, TOKEN_LIFETIME};
use crate::redis_handlers::{
    put_user_data_to_redis, get_user_data_by_email_from_redis, 
    get_user_data_by_id_from_redis, drop_user_data_from_redis
};
use crate::autorization::{JWToken, create_jwt};
use crate::convertations::{AsHash, AsBase64, FromBase64};
use crate::tools::{send_email, generate_random_string};

pub fn unauthorized_users_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/create_user")
                .route(web::post().to(create_new_user))
        ).service(
            web::resource("/authorisation")
                .route(web::post().to(check_user))
        ).service(
            web::resource("/forgot_password")
                .route(web::put().to(change_forgotten_password))
        ).service(
            web::resource("/user_verification/{user_id}/{verification_token}")
                .route(web::get().to(start_email_verification))
        ).service(
            web::resource("/email_verification/{email}/{user_id}/{verification_token}")
                .route(web::get().to(updated_email_verification))
        );
}

pub fn authorized_users_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/change_username")
                .route(web::put().to(change_user_name))
        ).service(
            web::resource("/change_password")
                .route(web::put().to(change_user_password)) 
        ).service(
            web::resource("/change_user_email")
                .route(web::put().to(change_user_email))
        ).service(
            web::resource("/logout")
                .route(web::delete().to(logout))
        );
}

// unauthorized user managing

async fn create_new_user(
    postgres_db: Data<PostgresDB>, 
    user_data: Json<CreateUserBody>) -> impl Responder {

    let CreateUserBody {name, email, password} = user_data.0;
    let password = password.as_hash();
    let current_time = chrono::offset::Utc::now().naive_utc();

    log::info!("New user creation request: name `{}`, email, `{}`", name, email);

    let db_link = &*postgres_db.db.lock().unwrap();
    let check_query = format!(
        "SELECT 
            id
           FROM {}.{}
          WHERE email = $1", 
        APP_SCHEMA, 
        USERS_TABLE
    );
    let check_result = sqlx::query(&check_query)
        .bind(email.clone())
        .fetch_one(db_link)
        .await;

    match check_result {
        Ok(_) => {
            HttpResponse::BadRequest().json(ServerResponse {
                status: 400, 
                message: format!("User with email {} already exists", email)
            })
        }, 
        Err(_) => {
            let insert_query = format!(
                "INSERT INTO {}.{} (name, email, passwd, verification_status_id, status_id, creation_time) 
                      VALUES ($1, $2, $3, 0, 0, $4) 
                   RETURNING id", 
                APP_SCHEMA, 
                USERS_TABLE
            );
            let insert_result = sqlx::query(&insert_query)
                .bind(name)
                .bind(email.clone())
                .bind(password.clone())
                .bind(current_time)
                .fetch_one(db_link)
                .await;

            match insert_result {
                Ok(new_user) => {
                    let new_user_id: i32 = new_user.get("id");
                    let verification_token = format!(
                        "{}{}{}", 
                        new_user_id, 
                        password.clone(), 
                        current_time.timestamp()
                    ).as_hash();
                    let message = format!("
                        Click this link to finish your verification {}/user_verification/{}/{}
                        ", 
                        crate::SERVICE_URL, 
                        new_user_id, 
                        verification_token
                    );

                    send_email(email, "New user activation", &message);
                    HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("User created")
                    })
                }, 
                Err(db_error) => {
                    log::error!("Database issue: {:?}", db_error);
                    HttpResponse::InternalServerError().json(ServerResponse {
                        status: 500, 
                        message: String::from("Internal server error")
                    })
                }
            }
        }
    }
}

async fn check_user(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    user_data: Json<UserCredentials>) -> impl Responder {
    
    let UserCredentials { email, password } = user_data.0;
    let password = password.as_hash();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let redis_data = get_user_data_by_email_from_redis(redis_conn, &email);
    if let Ok(cached_user_data) = redis_data {
        if password == cached_user_data.passwd {

            let user_id = cached_user_data.id;
            let current_time = chrono::offset::Utc::now().naive_utc().timestamp();
            let jwtoken = JWToken::new(user_id, current_time);
            let token = create_jwt(jwtoken);
            
            return HttpResponse::Ok().cookie(Cookie::new("x-auth", &token)).json(token);
        }
    }

    let query = format!(
        "SELECT 
            id, name, email, passwd, verification_status_id, status_id, creation_time
           FROM {}.{}
          WHERE email = $1 AND status_id = 1", 
        APP_SCHEMA, 
        USERS_TABLE
    );
    let result = sqlx::query(&query)
        .bind(email)
        .map(|row| {
            StoredUser{
                id: row.get("id"),
                name: row.get("name"),
                email: row.get("email"), 
                passwd: row.get("passwd"), 
                verification_status_id: row.get("verification_status_id"), 
                status_id: row.get("status_id"), 
                creation_time: row.get("creation_time")
            } 
        })
        .fetch_all(db_link)
        .await;

    match result {
        Ok(stored_users) => {
            if stored_users.len() > 0 {
                let stored_user = &stored_users[0];
                if password == stored_user.passwd.clone().unwrap() {
                    
                    let authorized_user = stored_user.get_user();
                    put_user_data_to_redis(redis_conn, authorized_user, None);

                    let current_time = chrono::offset::Utc::now().naive_utc().timestamp();
                    let jwtoken = JWToken::new(stored_user.id, current_time);
                    let token = create_jwt(jwtoken);
                    
                    HttpResponse::Ok().cookie(Cookie::new("x-auth", &token)).json(token)

                } else {
                    HttpResponse::BadRequest().json(ServerResponse {
                        status: 400, 
                        message: String::from("Invalid user credentials")
                    })
                }
            } else {
                HttpResponse::BadRequest().json(ServerResponse {
                    status: 400, 
                    message: String::from("Invalid user credentials")
                })
            }
        }, 
        Err(db_error) => {
            log::error!("Database issue: {:?}", db_error);
            HttpResponse::InternalServerError().json(ServerResponse {
                status: 500, 
                message: String::from("Internal server error")
            })
        }
    }
}

async fn change_forgotten_password(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<ChangeForgottenPasswordBody>) -> impl Responder {
    
    let ChangeForgottenPasswordBody { email } = request_data.0;
    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let query = format!(
        "SELECT 
            id, name, email, passwd, verification_status_id, status_id, creation_time
           FROM {}.{}
          WHERE email = $1 AND status_id = 1", 
        APP_SCHEMA, 
        USERS_TABLE
    );
    let result = sqlx::query(&query)
        .bind(email.clone())
        .map(|row| {
            StoredUser{
                id: row.get("id"),
                name: row.get("name"),
                email: row.get("email"), 
                passwd: row.get("passwd"), 
                verification_status_id: row.get("verification_status_id"), 
                status_id: row.get("status_id"), 
                creation_time: row.get("creation_time")
            } 
        })
        .fetch_all(db_link)
        .await;

    match result {
        Ok(stored_users) => {
            if stored_users.len() > 0 {
                let mut stored_user = stored_users[0].get_user();

                let generated_password = generate_random_string();
                stored_user.passwd = generated_password.clone().as_hash();
                stored_user.verification_status_id = 3;

                put_user_data_to_redis(redis_conn, stored_user, Some(43200));
                let message = format!(
                    "Your new temporary password {}. Would be valid in next 12 hours.", 
                    generated_password
                );
                // send_verification_mail(email, 0, &message);
                send_email(email, "Password reset email", &message);

                HttpResponse::Ok().json(ServerResponse {
                    status: 200, 
                    message: String::from("Email with new password sent")
                })
                
            } else {
                HttpResponse::BadRequest().json(ServerResponse {
                    status: 400, 
                    message: String::from("Invalid email")
                })
            }
        }, 
        Err(db_error) => {
            log::error!("Database issue: {:?}", db_error);
            HttpResponse::InternalServerError().json(ServerResponse {
                status: 500, 
                message: String::from("Internal server error")
            })
        }
    }
}

async fn start_email_verification(
    postgres_db: Data<PostgresDB>, 
    request_path: web::Path<(i32, String)>) -> impl Responder {

    let (user_id, verification_token) = request_path.into_inner();
    let db_link = &*postgres_db.db.lock().unwrap();

    let check_query = format!("
         SELECT 
            passwd, creation_time
           FROM {}.{}
          WHERE id = $1 
            AND status_id = 0
            ", 
        APP_SCHEMA, 
        USERS_TABLE
    );
    let check_result = sqlx::query(&check_query)
        .bind(user_id)
        .fetch_one(db_link)
        .await;

    match check_result {
        Ok(row) => { // idle user found
            let password: String = row.get("passwd");
            let creation_time: NaiveDateTime = row.get("creation_time");

            let stored_token = format!(
                "{}{}{}", 
                user_id, 
                password.clone(), 
                creation_time.timestamp()
            ).as_hash();

            if stored_token == verification_token { // verification passed
                let update_query = format!("
                    UPDATE {}.{}
                       SET status_id = 1, verification_status_id = 1
                     WHERE id = $1 
                       AND status_id = 0
                    ", 
                    APP_SCHEMA, 
                    USERS_TABLE
                ); 
                let update_result = sqlx::query(&update_query)
                    .bind(user_id)
                    .execute(db_link)
                    .await;
                
                match update_result {
                    Ok(_) => {
                        HttpResponse::Ok().body("Ok")
                    }, 
                    Err(db_error) => {
                        log::error!("Database issue: {:?}", db_error);
                        HttpResponse::InternalServerError().json(ServerResponse {
                            status: 500, 
                            message: String::from("Internal server error")
                        })
                    }
                }
            } else { // invalid token
                HttpResponse::BadRequest().body("BadRequest")
            }
        }, 
        Err(_) => { // no one idle user found
            HttpResponse::BadRequest().body("BadRequest")
        }
    }

}

async fn updated_email_verification(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_path: web::Path<(String, i32, String)>) -> impl Responder {

    let (encoded_email, user_id, verification_token) = request_path.into_inner();
    match encoded_email.from_base64() {
        Some(new_email) => {
            let db_link = &*postgres_db.db.lock().unwrap();
            let redis_conn = &mut *redis_db.db.lock().unwrap();

            let check_query = format!("
                SELECT 
                    email, creation_time
                  FROM {}.{}
                 WHERE id = $1 
                   AND status_id = 1
                   AND verification_status_id = 4
                    ", 
                APP_SCHEMA, 
                USERS_TABLE
            );
            let check_result = sqlx::query(&check_query)
                .bind(user_id)
                .fetch_one(db_link)
                .await;

            match check_result {
                Ok(row) => { // idle user found
                    let current_email: String = row.get("email");
                    let creation_time: NaiveDateTime = row.get("creation_time");

                    let stored_token = format!(
                        "{}{}{}", 
                        new_email, 
                        current_email, 
                        creation_time.timestamp()
                    ).as_hash();

                    if stored_token == verification_token { // verification passed
                        let update_query = format!("
                            UPDATE {}.{}
                               SET email = $2, verification_status_id = 1
                             WHERE id = $1 
                               AND status_id = 1
                               AND verification_status_id = 4
                            ", 
                            APP_SCHEMA, 
                            USERS_TABLE
                        ); 
                        let update_result = sqlx::query(&update_query)
                            .bind(user_id)
                            .bind(new_email)
                            .execute(db_link)
                            .await;
                        
                        match update_result {
                            Ok(_) => {
                                drop_user_data_from_redis(redis_conn, user_id);
                                HttpResponse::Ok().body("Ok")
                            }, 
                            Err(db_error) => {
                                log::error!("Database issue: {:?}", db_error);
                                HttpResponse::InternalServerError().json(ServerResponse {
                                    status: 500, 
                                    message: String::from("Internal server error")
                                })
                            }
                        }
                    } else { // invalid token
                        HttpResponse::BadRequest().body("BadRequest")
                    }
                }, 
                Err(_) => { // no one idle user found
                    HttpResponse::BadRequest().body("BadRequest")
                }
            }
        }, 
        None => {
            HttpResponse::BadRequest().body("BadRequest")
        }
    }
}

// authorized user managing

async fn change_user_name(
    request: HttpRequest,
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<ChangeUsernameBody>) -> impl Responder {
    
    let ChangeUsernameBody {new_name} = request_data.0;
    let headers = request.headers();
    let user_id: i32 = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let query = format!("
        UPDATE {}.{}
            SET name = $2
            WHERE id = $1 
            AND status_id = 1
        ", 
        APP_SCHEMA, 
        USERS_TABLE
    );
    let result = sqlx::query(&query)
        .bind(user_id)
        .bind(new_name)
        .execute(db_link)
        .await;

    match result {
        Ok(_) => {
            drop_user_data_from_redis(redis_conn, user_id);
            if headers.contains_key("new_token") {
                let token = headers.get("new_token").unwrap().to_str().unwrap();
                HttpResponse::Ok().cookie(Cookie::new("x-auth", token)).json(ServerResponse {
                    status: 200, 
                    message: String::from("User name updated")
                })
            } else {
                HttpResponse::Ok().json(ServerResponse {
                    status: 200, 
                    message: String::from("User name updated")
                })
            }
        }, 
        Err(db_error) => {
            log::error!("Database issue: {:?}", db_error);
            HttpResponse::InternalServerError().json(ServerResponse {
                status: 500, 
                message: String::from("Internal server error")
            })
        }
    }
}

async fn change_user_password(
    request: HttpRequest,
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<ChangePasswordBody>) -> impl Responder {

    let ChangePasswordBody {old_password, new_password} = request_data.0;
    let headers = request.headers();
    let user_id: i32 = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    let old_password = old_password.as_hash();
    let new_password = new_password.as_hash();
    
    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let query: String;        
    let redis_data = get_user_data_by_id_from_redis(redis_conn, user_id);
    if let Ok(cached_user_data) = redis_data {
        if old_password == cached_user_data.passwd {
            query = format!("
                UPDATE {}.{}
                    SET passwd = $2, verification_status_id = 1
                    WHERE id = $1 
                    AND status_id = 1
                RETURNING id
                ", 
                APP_SCHEMA, 
                USERS_TABLE
            );
        } else {
            return HttpResponse::BadRequest().json(ServerResponse {
                status: 400, 
                message: String::from("Invalid password")
            });
        }
    } else {
        query = format!("
            UPDATE {}.{}
                SET passwd = $2
                WHERE id = $1 
                AND status_id = 1
                AND passwd = '{}'
            RETURNING id
            ", 
            APP_SCHEMA, 
            USERS_TABLE, 
            old_password
        ); 
    }

    let result = sqlx::query(&query)
        .bind(user_id)
        .bind(new_password)
        .fetch_all(db_link)
        .await;

    match result {
        Ok(update_result) => {
            if update_result.len() > 0 {
                drop_user_data_from_redis(redis_conn, user_id);

                if headers.contains_key("new_token") {
                    let token = headers.get("new_token").unwrap().to_str().unwrap();
                    HttpResponse::Ok().cookie(Cookie::new("x-auth", token)).json(ServerResponse {
                        status: 200, 
                        message: String::from("Password updated")
                    })
                } else {
                    HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Password updated")
                    })
                }
            } else {
                HttpResponse::BadRequest().json(ServerResponse {
                    status: 400, 
                    message: String::from("Invalid password")
                })
            }
        }, 
        Err(db_error) => {
            log::error!("Database issue: {:?}", db_error);
            HttpResponse::InternalServerError().json(ServerResponse {
                status: 500, 
                message: String::from("Internal server error")
            })
        }
    }
    
}

async fn change_user_email(
    request: HttpRequest,
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<ChangeEmailBody>) -> impl Responder {

    let ChangeEmailBody {new_email } = request_data.0;
    let headers = request.headers();
    let user_id: i32 = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let check_query = format!(
        "SELECT 
            id
           FROM {}.{}
          WHERE email = $1", 
        APP_SCHEMA, 
        USERS_TABLE
    );
    let check_result = sqlx::query(&check_query)
        .bind(new_email.clone())
        .fetch_all(db_link)
        .await;

    match check_result {
        Ok(existed_users) => {
            if existed_users.len() > 0 {
                HttpResponse::BadRequest().json(ServerResponse {
                    status: 400, 
                    message: format!("User with email {} already exists", new_email)
                })
            } else {
                let query = format!("
                    UPDATE {}.{}
                        SET verification_status_id = 4
                        WHERE id = $1 
                        AND status_id = 1
                    RETURNING id, name, email, passwd, verification_status_id, status_id, creation_time
                    ", 
                    APP_SCHEMA, 
                    USERS_TABLE
                );
                let result = sqlx::query(&query)
                    .bind(user_id)
                    .map(|row| {
                        StoredUser{
                            id: row.get("id"),
                            name: row.get("name"),
                            email: row.get("email"), 
                            passwd: row.get("passwd"), 
                            verification_status_id: row.get("verification_status_id"), 
                            status_id: row.get("status_id"), 
                            creation_time: row.get("creation_time")
                        } 
                    })
                    .fetch_all(db_link)
                    .await;

                match result {
                    Ok(stored_users) => {
                        if stored_users.len() > 0 {
                            let stored_user = &stored_users[0].get_user();
                            let verification_token = format!(
                                "{}{}{}", 
                                new_email, 
                                stored_user.email, 
                                stored_user.creation_time
                            ).as_hash();

                            drop_user_data_from_redis(redis_conn, user_id);
                            let message = format!(
                                "Click the link to verify your new email address {}/email_verification/{}/{}/{}", 
                                crate::SERVICE_URL, 
                                new_email.as_base64(), 
                                user_id, 
                                verification_token
                            );
                            // send_verification_mail(new_email, user_id, &message);
                            send_email(new_email, "New email address verification", &message);

                            if headers.contains_key("new_token") {
                                let token = headers.get("new_token").unwrap().to_str().unwrap();
                                HttpResponse::Ok().cookie(Cookie::new("x-auth", token)).json(ServerResponse {
                                    status: 200, 
                                    message: String::from("Verification mail was sent")
                                })
                            } else {
                                HttpResponse::Ok().json(ServerResponse {
                                    status: 200, 
                                    message: String::from("Verification mail was sent")
                                })
                            }
                        } else {
                            HttpResponse::InternalServerError().json(ServerResponse {
                                status: 500, 
                                message: String::from("Internal server error")
                            })
                        }
                    }, 
                    Err(db_error) => {
                        log::error!("Database issue: {:?}", db_error);
                        HttpResponse::InternalServerError().json(ServerResponse {
                            status: 500, 
                            message: String::from("Internal server error")
                        })
                    }
                }
            }
        }, 
        Err(db_error) => {
            log::error!("Database issue: {:?}", db_error);
            HttpResponse::InternalServerError().json(ServerResponse {
                status: 500, 
                message: String::from("Internal server error")
            })
        }
    }
}

async fn logout(
    request: HttpRequest,
    redis_db: Data<RedisDB>) -> impl Responder {
    
    let headers = request.headers();
    let user_id: i32 = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    let redis_conn = &mut *redis_db.db.lock().unwrap();
    drop_user_data_from_redis(redis_conn, user_id);

    let mut cookie = Cookie::new("x-auth", "");
    cookie.set_max_age(Duration::seconds(0));

    HttpResponse::Ok().cookie(cookie).body("Logout")
}
