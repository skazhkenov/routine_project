use actix_web::{
    web::{self, Data, Json}, 
    Responder, HttpResponse, 
    cookie::{time::Duration, Cookie}
};
use chrono::{self, NaiveDateTime};
use log;
use sqlx::{self, Row};
use uuid::Uuid;

use crate::models::{
    ServerResponse, UserCredentials, CreateUserBody, 
    ChangeForgottenPasswordBody, StoredUser
};
use crate::{PersistentDB, CacheDB, APP_SCHEMA, USERS_TABLE, TOKEN_LIFETIME};
use crate::redis_handlers::{
    put_user_data_to_redis, 
    get_user_data_by_email_from_redis, 
    drop_user_data_from_redis
};
use crate::autorization::{JWToken, create_jwt};
use crate::convertations::{AsHash, FromBase64};
use crate::tools::{send_email, generate_random_password, is_valid_password, is_valid_email};

pub fn unauthorized_users_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/create_user")
                .route(web::post().to(handle_create_user))
        ).service(
            web::resource("/authorization")
                .route(web::post().to(handle_authorization))
        ).service(
            web::resource("/forgot_password")
                .route(web::put().to(handle_forgot_password))
        ).service(
            web::resource("/user_verification/{user_id}/{verification_token}")
                .route(web::get().to(handle_user_verification))
        ).service(
            web::resource("/email_verification/{email}/{user_id}/{verification_token}")
                .route(web::get().to(handle_email_verification))
        );
}

async fn handle_create_user(
    postgres_db: Data<PersistentDB>, 
    user_data: Json<CreateUserBody>) -> impl Responder {

    let CreateUserBody {name, email, password} = user_data.0;
    log::info!("New user creation request: name `{}`, email, `{}`", name, email);

    if !is_valid_password(&password) {
        log::warn!("Invalid password received");
        return HttpResponse::BadRequest().body("Invalid password");
    }
    if !is_valid_email(&email) {
        log::warn!("Invalid email received: `{}`", email);
        return HttpResponse::BadRequest().body("Invalid email");
    }

    let password = password.as_hash();
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
            log::warn!("Attempt to create new account with email existed in DB: `{}`", email);
            HttpResponse::BadRequest().json(ServerResponse {
                status: 400, 
                message: format!("User with email {} already exists", email)
            })
        }, 
        Err(_) => {
            let insert_query = format!(
                "INSERT INTO {}.{} (name, email, passwd, verification_status_id, status_id) 
                      VALUES ($1, $2, $3, 0, 0) 
                   RETURNING id, created_at", 
                APP_SCHEMA, 
                USERS_TABLE
            );
            let insert_result = sqlx::query(&insert_query)
                .bind(name)
                .bind(email.clone())
                .bind(password.clone())
                .fetch_one(db_link)
                .await;

            match insert_result {
                Ok(new_user) => {
                    let new_user_id: Uuid = new_user.get("id");
                    let creation_time: NaiveDateTime = new_user.get("created_at");
                    let verification_token = format!(
                        "{}{}{}", 
                        new_user_id, 
                        password.clone(), 
                        creation_time.timestamp()
                    ).as_hash();
                    let message = format!("
                        Click this link to finish your verification {}/user_verification/{}/{}
                        ", 
                        crate::SERVICE_URL, 
                        new_user_id, 
                        verification_token
                    );

                    send_email(&email, "New user activation", &message);
                    log::info!("Verification email for new user sent to address: `{}`", email);
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

async fn handle_authorization(
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    user_data: Json<UserCredentials>) -> impl Responder {
    
    let UserCredentials { email, password } = user_data.0;
    let password = password.as_hash();

    log::info!("User login request with email: `{}`", email);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let redis_data = get_user_data_by_email_from_redis(redis_conn, &email);
    if let Ok(cached_user_data) = redis_data {
        if password == cached_user_data.passwd {

            let user_id = cached_user_data.id;
            let current_time = chrono::offset::Utc::now().naive_utc().timestamp();
            let jwtoken = JWToken::new(user_id, current_time);
            let token = create_jwt(jwtoken);
            let mut cookie = Cookie::new("x-auth", &token);
            cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));
            
            log::info!("User: `{}` have been authorized", user_id);
            return HttpResponse::Ok().cookie(cookie).json(token);
        }
    }

    let query = format!(
        "SELECT 
            id, name, email, passwd, verification_status_id, status_id, created_at, updated_at
           FROM {}.{}
          WHERE email = $1 AND status_id = 1", 
        APP_SCHEMA, 
        USERS_TABLE
    );
    let result = sqlx::query(&query)
        .bind(&email)
        .map(|row| {
            StoredUser{
                id: row.get("id"),
                name: row.get("name"),
                email: row.get("email"), 
                passwd: row.get("passwd"), 
                verification_status_id: row.get("verification_status_id"), 
                status_id: row.get("status_id"), 
                created_at: row.get("created_at"), 
                updated_at: row.get("updated_at")
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
                    let mut cookie = Cookie::new("x-auth", &token);
                    cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                    log::info!("User: `{}` have been authorized", stored_user.id);
                    HttpResponse::Ok().cookie(cookie).json(token)

                } else {
                    log::warn!("Invalid password received from user with email: `{}`", email);
                    HttpResponse::BadRequest().json(ServerResponse {
                        status: 400, 
                        message: String::from("Invalid user credentials")
                    })
                }
            } else {
                log::warn!("Invalid email received: `{}`", email);
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

async fn handle_forgot_password(
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    request_data: Json<ChangeForgottenPasswordBody>) -> impl Responder {
    
    let ChangeForgottenPasswordBody { email } = request_data.0;
    log::info!("Request for change forgotten password for user with email: `{}`", email);
    if !is_valid_email(&email) {
        log::warn!("Invalid email received: `{}`", email);
        return HttpResponse::BadRequest().body("Invalid email");
    }

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let query = format!(
        "SELECT 
            id, name, email, passwd, verification_status_id, status_id, created_at, updated_at
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
                created_at: row.get("created_at"), 
                updated_at: row.get("updated_at")
            } 
        })
        .fetch_all(db_link)
        .await;

    match result {
        Ok(stored_users) => {
            if stored_users.len() > 0 {
                let mut stored_user = stored_users[0].get_user();

                let generated_password = generate_random_password();
                stored_user.passwd = generated_password.clone().as_hash();
                stored_user.verification_status_id = 3;

                put_user_data_to_redis(redis_conn, stored_user, Some(43200));
                let message = format!(
                    "Your new temporary password {}. Would be valid in next 12 hours.", 
                    generated_password
                );
                send_email(&email, "Password reset email", &message);

                log::info!("Message with temporary password sent to address: {}", email);
                HttpResponse::Ok().json(ServerResponse {
                    status: 200, 
                    message: String::from("Email with new password sent")
                })
                
            } else {
                log::warn!("Unexisted email received: `{}`", email);
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

async fn handle_user_verification(
    postgres_db: Data<PersistentDB>, 
    request_path: web::Path<(Uuid, String)>) -> impl Responder {

    let (user_id, verification_token) = request_path.into_inner();
    log::info!("Account activation request from user: `{}`", user_id);
    let db_link = &*postgres_db.db.lock().unwrap();

    let check_query = format!("
         SELECT 
            passwd, created_at
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
            let creation_time: NaiveDateTime = row.get("created_at");

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
                        log::info!("Account of user `{}` activated", user_id);
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
                log::warn!("Invalid verification token received from user: `{}`", user_id);
                HttpResponse::BadRequest().body("BadRequest")
            }
        }, 
        Err(_) => { // no one idle user found
            log::warn!("User `{}` attemted to activate its non-idle account", user_id);
            HttpResponse::BadRequest().body("BadRequest")
        }
    }

}

async fn handle_email_verification(
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    request_path: web::Path<(String, Uuid, String)>) -> impl Responder {

    let (encoded_email, user_id, verification_token) = request_path.into_inner();
    log::info!("New email verification request from user: `{}`", user_id);
    match encoded_email.from_base64() {
        Some(new_email) => {
            let db_link = &*postgres_db.db.lock().unwrap();
            let redis_conn = &mut *redis_db.db.lock().unwrap();

            let check_query = format!("
                SELECT 
                    email, created_at
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
                    let creation_time: NaiveDateTime = row.get("created_at");

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
                            .bind(&new_email)
                            .execute(db_link)
                            .await;
                        
                        match update_result {
                            Ok(_) => {
                                drop_user_data_from_redis(redis_conn, user_id);
                                log::info!("New email `{}` setted for user: `{}`", new_email, user_id);
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
                        log::warn!("Invalid verification token received from user: `{}`", user_id);
                        HttpResponse::BadRequest().body("BadRequest")
                    }
                }, 
                Err(_) => { // no one idle user found
                    log::warn!("User `{}` attempted verify email: `{}`", user_id, new_email);
                    HttpResponse::BadRequest().body("BadRequest")
                }
            }
        }, 
        None => {
            log::warn!("Invalid base64 decoded email received: `{}`, user: `{}`", encoded_email, user_id);
            HttpResponse::BadRequest().body("BadRequest")
        }
    }
}

