use actix_web::{web::{self, Data, Json}, Responder, HttpResponse};
use serde::{Serialize, Deserialize};
use sqlx::{self, Row};

use redis::Commands;
use chrono::{self, NaiveDateTime};
use log;

use crate::models::{
    ServerResponse, User, UserCredentials, CreateUserBody, ChangePasswordBody, 
    ChangeForgottenPasswordBody, ChangeEmailBody, ChangeUsernameBody, DeleteUserBody, StoredUser
};
use crate::{PostgresDB, RedisDB, APP_SCHEMA, USERS_TABLE};
use crate::redis_handlers::{
    put_user_data_to_redis, get_user_data_by_email_from_redis, get_user_data_by_id_from_redis, get_user_data_lifetime_from_redis, 
    drop_user_data_from_redis, put_user_token_to_redis, get_user_token_from_redis, drop_user_token_from_redis
};
use crate::convertations::{AsHash, AsBase64, FromBase64};

#[derive(Serialize, Deserialize)]
pub struct UserWebData {
    pub user_id: i32, 
    pub token: String
}

impl UserWebData {
    pub fn id(&self) -> i32 {
        self.user_id
    } 
    pub fn token(&self) -> String {
        self.token.clone()
    }
}

pub fn make_token(password: String) -> String {
    let current_time = chrono::offset::Utc::now().naive_utc();
    let token = format!("{}{}", password, current_time).as_hash();

    token
}

pub fn is_valid_token(conn: &mut redis::Connection, web_token_data: UserWebData) -> bool {

    let user_id = web_token_data.id();
    let token = web_token_data.token();

    if let Ok(stored_token) = get_user_token_from_redis(conn, user_id) {
        if stored_token == token {
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn send_verification_mail(email: String, user_id: i32, message: &str) {
    
    println!("Verification mail for user {} sent to {}. {}", user_id, email, message);
}

pub fn users_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/create_user")
                .route(web::post().to(create_new_user))
        ).service(
            web::resource("/authorisation")
                .route(web::post().to(check_user))
        ).service(
            web::resource("/change_username")
                .route(web::put().to(change_user_name))
        ).service(
            web::resource("/change_password")
                .route(web::put().to(change_user_password)) 
        ).service(
            web::resource("/change_user_email")
                .route(web::put().to(change_user_email))
        ).service(
            web::resource("/forgot_password")
                .route(web::put().to(change_forgotten_password))
        ).service(
            web::resource("/user_verification/{user_id}/{verification_token}")
                .route(web::get().to(start_email_verification))
        ).service(
            web::resource("/email_verification/{email}/{user_id}/{verification_token}")
                .route(web::get().to(updated_email_verification))
        ).service(
            web::resource("/logout")
                .route(web::delete().to(logout))
        );
}

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
                    send_verification_mail(email, new_user_id, &message);
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
            let token = make_token(password.clone());

            if cached_user_data.verification_status_id == 3 {
                let lifetime_rest_request = get_user_data_lifetime_from_redis(redis_conn, user_id);
                match lifetime_rest_request {
                    Ok(lifetime_rest) => {
                        put_user_token_to_redis(redis_conn, user_id, token.clone(), Some(lifetime_rest));
                    }, 
                    Err(_) => {
                        put_user_token_to_redis(redis_conn, user_id, token.clone(), Some(0));
                    }
                }
            } else {
                put_user_token_to_redis(redis_conn, user_id, token.clone(), None);
            }

            let user_token_json = UserWebData {
                user_id, 
                token
            };
            return HttpResponse::Ok().json(user_token_json);
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
                    
                    let authorised_user = stored_user.get_user();
                    put_user_data_to_redis(redis_conn, authorised_user, None);
                    
                    let token = make_token(password.clone());
                    put_user_token_to_redis(redis_conn, stored_user.id, token.clone(), None);

                    let user_token_json = UserWebData {
                        user_id: stored_user.id, 
                        token: token
                    };
                    HttpResponse::Ok().json(user_token_json)
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

async fn change_user_name(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<ChangeUsernameBody>) -> impl Responder {
    
    let ChangeUsernameBody {user_data, new_name} = request_data.0;
    let user_id = user_data.id();
    let token = user_data.token();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {

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
                HttpResponse::Ok().json(ServerResponse {
                    status: 200, 
                    message: String::from("User name updated")
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
    } else {
        log::warn!("Unauthorised user {} tried to get access", user_id);
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
}

async fn change_user_password(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<ChangePasswordBody>) -> impl Responder {

    let ChangePasswordBody {user_data, old_password, new_password} = request_data.0;
    let old_password = old_password.as_hash();
    let new_password = new_password.as_hash();
    let user_id = user_data.id();
    let token = user_data.token();
    
    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
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
                    drop_user_token_from_redis(redis_conn, user_id);

                    HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Password updated")
                    })
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
    } else {
        log::warn!("Unauthorised user {} tried to get access", user_id);
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
    
}

async fn change_user_email(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<ChangeEmailBody>) -> impl Responder {

    let ChangeEmailBody { user_data, new_email } = request_data.0;
    let user_id = user_data.id();
    let token = user_data.token();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
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
                        "{}/email_verification/{}/{}/{}", 
                        crate::SERVICE_URL, 
                        new_email.as_base64(), 
                        user_id, 
                        verification_token
                    );
                    send_verification_mail(new_email, user_id, &message);

                    HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Verification mail was sent")
                    })
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
    } else {
        log::warn!("Unauthorised user {} tried to get access", user_id);
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
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

                let generated_password = String::from("123");
                stored_user.passwd = generated_password.clone().as_hash();
                stored_user.verification_status_id = 3;

                put_user_data_to_redis(redis_conn, stored_user, Some(43200));
                let message = format!(
                    "Your new temporary password {}. Would be valid in next 12 hours.", 
                    generated_password
                );
                send_verification_mail(email, 0, &message);

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
                       SET status_id = 1
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
                                drop_user_token_from_redis(redis_conn, user_id);
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

async fn logout(
    redis_db: Data<RedisDB>, 
    user_data: Json<UserWebData>) -> impl Responder {
    
    let UserWebData { user_id, token } = user_data.0;
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
        drop_user_data_from_redis(redis_conn, user_id);
        drop_user_token_from_redis(redis_conn, user_id);
        HttpResponse::Ok().json(ServerResponse {
            status: 200, 
            message: String::from("Logout")
        })
    } else {
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
}

