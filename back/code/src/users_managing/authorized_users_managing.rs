use actix_web::{
    web::{self, Data, Json}, 
    HttpRequest, Responder, HttpResponse, 
    cookie::{time::Duration, Cookie}
};
use log;
use sqlx::{self, Row};
use uuid::Uuid;

use crate::models::{
    Profile, ServerResponse, ChangePasswordBody, 
    ChangeEmailBody, ChangeUsernameBody, StoredUser
};
use crate::{PersistentDB, CacheDB, APP_SCHEMA, USERS_TABLE, TOKEN_LIFETIME};
use crate::redis_handlers::{
    put_user_data_to_redis, get_user_data_by_id_from_redis, drop_user_data_from_redis
};
use crate::convertations::{AsHash, AsBase64};
use crate::tools::{send_email, is_valid_password, is_valid_email};

pub fn authorized_users_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/get_user")
                .route(web::get().to(handle_get_user))
        ).service(
            web::resource("/change_username")
                .route(web::put().to(handle_change_username))
        ).service(
            web::resource("/change_password")
                .route(web::put().to(handle_change_password)) 
        ).service(
            web::resource("/change_email")
                .route(web::put().to(handle_change_email))
        ).service(
            web::resource("/logout")
                .route(web::delete().to(handle_logout))
        );
}

async fn handle_get_user(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>) -> impl Responder {

    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();
    log::info!("Requested profile data for user: `{}`", user_id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let redis_data = get_user_data_by_id_from_redis(redis_conn, user_id);
    if let Ok(cached_user_data) = redis_data {
        let username: String = cached_user_data.name;
        let email: String = cached_user_data.email;

        if headers.contains_key("new_token") {
            let token = headers.get("new_token").unwrap().to_str().unwrap();
            let mut cookie = Cookie::new("x-auth", token);
            cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

            return HttpResponse::Ok().cookie(cookie).json(Profile {
                id: user_id,
                name: username, 
                email: email
            });
        } else {
            return HttpResponse::Ok().json(Profile {
                id: user_id,
                name: username, 
                email: email
            });
        }
    }

    let query = format!("
        SELECT 
            id, name, email
        FROM {}.{}
        WHERE status_id = 1
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
                put_user_data_to_redis(redis_conn, stored_user.get_user(), None);

                let user = stored_user.get_user();
                let username: String = user.name;
                let email: String = user.email;
                if headers.contains_key("new_token") {
                    let token = headers.get("new_token").unwrap().to_str().unwrap();
                    let mut cookie = Cookie::new("x-auth", token);
                    cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));
        
                    HttpResponse::Ok().cookie(cookie).json(Profile {
                        id: user_id,
                        name: username, 
                        email: email
                    })
                } else {
                    HttpResponse::Ok().json(Profile {
                        id: user_id,
                        name: username, 
                        email: email
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

async fn handle_change_username(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    request_data: Json<ChangeUsernameBody>) -> impl Responder {
    
    let ChangeUsernameBody {new_name} = request_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();
    log::info!("Request for changing name from user: `{}`", user_id);

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
        .bind(&new_name)
        .execute(db_link)
        .await;

    match result {
        Ok(_) => {
            drop_user_data_from_redis(redis_conn, user_id);
            log::info!("New name `{}` setted for user: `{}`", new_name, user_id);

            if headers.contains_key("new_token") {
                let token = headers.get("new_token").unwrap().to_str().unwrap();
                let mut cookie = Cookie::new("x-auth", token);
                cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                HttpResponse::Ok().cookie(cookie).json(ServerResponse {
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

async fn handle_change_password(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    request_data: Json<ChangePasswordBody>) -> impl Responder {

    let ChangePasswordBody {old_password, new_password} = request_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();
    
    log::info!("Request for changing password from user: `{}`", user_id);
    if !is_valid_password(&new_password) {
        log::warn!("Invalid new password received from user: `{}`", user_id);
        return HttpResponse::BadRequest().body("Invalid password");
    }

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
            log::warn!("Invalid current password received from user: `{}`", user_id);
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
                log::info!("Password updated for user: `{}`", user_id);

                let mut cookie = Cookie::new("x-auth", "");
                cookie.set_max_age(Duration::seconds(0));

                HttpResponse::Ok().cookie(cookie).json(ServerResponse {
                    status: 200, 
                    message: String::from("Password updated")
                })

            } else {
                log::warn!("Invalid current password received from user: `{}`", user_id);
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

async fn handle_change_email(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    request_data: Json<ChangeEmailBody>) -> impl Responder {

    let ChangeEmailBody {new_email } = request_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    log::info!("Request for changing email to: `{}` from user: `{}`", new_email, user_id);
    if !is_valid_email(&new_email) {
        log::warn!("Invalid email: `{}` received from user: `{}`", new_email, user_id);
        return HttpResponse::BadRequest().body("Invalid email");
    }

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
                log::warn!("User `{}` attempted to set email as new witch exists in DB: `{}`", user_id, new_email);
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
                    RETURNING id, name, email, passwd, verification_status_id, status_id, created_at, updated_at
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
                            created_at: row.get("created_at"), 
                            updated_at: row.get("updated_at")
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
                                stored_user.created_at
                            ).as_hash();

                            drop_user_data_from_redis(redis_conn, user_id);
                            let message = format!(
                                "Click the link to verify your new email address {}/email_verification/{}/{}/{}", 
                                crate::SERVICE_URL, 
                                new_email.as_base64(), 
                                user_id, 
                                verification_token
                            );
                            send_email(&new_email, "New email address verification", &message);
                            log::info!("Verification email for user `{}` sent to address: `{}`", user_id, new_email);

                            if headers.contains_key("new_token") {
                                let token = headers.get("new_token").unwrap().to_str().unwrap();
                                let mut cookie = Cookie::new("x-auth", token);
                                cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                                HttpResponse::Ok().cookie(cookie).json(ServerResponse {
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

async fn handle_logout(
    request: HttpRequest,
    redis_db: Data<CacheDB>) -> impl Responder {
    
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    let redis_conn = &mut *redis_db.db.lock().unwrap();
    drop_user_data_from_redis(redis_conn, user_id);

    let mut cookie = Cookie::new("x-auth", "");
    cookie.set_max_age(Duration::seconds(0));

    log::info!("Logout of user: `{}`", user_id);
    HttpResponse::Ok().cookie(cookie).body("Logout")
}
