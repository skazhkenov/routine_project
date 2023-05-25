use actix_web::{
    web::{self, Data, Json}, 
    Responder, HttpRequest, HttpResponse, 
    cookie::{time::Duration, Cookie}
};
use log;
use sqlx::{self, Row};
use uuid::Uuid;

use crate::{PersistentDB, CacheDB, APP_SCHEMA, BOARDS_TABLE, TOKEN_LIFETIME};
use crate::redis_handlers::{
    get_user_boards_from_redis, 
    put_user_boards_to_redis, 
    drop_user_boards_from_redis
};
use crate::models::{
    ServerResponse, Board, StoredBoard, 
    CreateBoardBody, UpdateBoardBody, DeleteBoardBody
};

pub fn boards_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/user_boards")
                .route(web::get().to(handle_user_boards))
        )
        .service(
            web::resource("/create_board")
                .route(web::post().to(handle_create_board))
        )
        .service(
            web::resource("/change_board")
                .route(web::put().to(handle_change_board))
        )
        .service(
            web::resource("/delete_board")
                .route(web::delete().to(handle_delete_board))
        );
}

async fn handle_user_boards(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>) -> impl Responder {

    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    log::info!("Boards requested by user {}", user_id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let redis_data = get_user_boards_from_redis(redis_conn, user_id);
    if let Ok(redis_boards_list) = redis_data {
        if redis_boards_list.len() > 0 {
            if headers.contains_key("new_token") {
                let token = headers.get("new_token").unwrap().to_str().unwrap();
                let mut cookie = Cookie::new("x-auth", token);
                cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                return HttpResponse::Ok().cookie(cookie).json(redis_boards_list);
            } else {
                return HttpResponse::Ok().json(redis_boards_list);
            }
        }
    }

    let query = format!(
        "SELECT 
            id, title, description, creation_time    
            FROM {}.{}
            WHERE status_id = 0 AND owner_id = $1
            ORDER BY creation_time", 
        APP_SCHEMA, 
        BOARDS_TABLE
    );
    let result = sqlx::query(&query)
        .bind(user_id)
        .map(|row| {
            StoredBoard{
                id: row.get("id"),
                title: row.get("title"),
                description: row.get("description"),
                creation_time: row.get("creation_time")
            } 
        })
        .fetch_all(db_link)
        .await;

    match result {
        Ok(stored_boards_list) => {
            
            let mut board_list: Vec<Board> = vec![];
            for stored_board in stored_boards_list.iter() {
                let board_to_return = stored_board.get_board();
                board_list.push(board_to_return);
            }
            put_user_boards_to_redis(redis_conn, user_id, &board_list);

            if headers.contains_key("new_token") {
                let token = headers.get("new_token").unwrap().to_str().unwrap();
                let mut cookie = Cookie::new("x-auth", token);
                cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                HttpResponse::Ok().cookie(cookie).json(board_list)
            } else {
                HttpResponse::Ok().json(board_list)
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

async fn handle_create_board(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    board_data: Json<CreateBoardBody>) -> impl Responder {

    let CreateBoardBody{title, description} = board_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    log::info!("Creation new board by user {}", user_id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let query = format!(
        "INSERT INTO {}.{} (title, description, status_id, owner_id) 
                VALUES ($1, $2, 0, $3)", 
        APP_SCHEMA, 
        BOARDS_TABLE
    );
    let result = sqlx::query(&query)
        .bind(title)
        .bind(description)
        .bind(user_id)
        .execute(db_link)
        .await;

    drop_user_boards_from_redis(redis_conn, user_id);
    match result {
        Ok(_) => {

            if headers.contains_key("new_token") {
                let token = headers.get("new_token").unwrap().to_str().unwrap();
                let mut cookie = Cookie::new("x-auth", token);
                cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                HttpResponse::Ok().cookie(cookie).json(ServerResponse {
                    status: 200, 
                    message: String::from("Board created")
                })
            } else {
                HttpResponse::Ok().json(ServerResponse {
                    status: 200, 
                    message: String::from("Board created")
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

async fn handle_change_board(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    board_data: Json<UpdateBoardBody>) -> impl Responder {

    let UpdateBoardBody{id, title, description} = board_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    log::info!("User {} tried to change board {}", user_id, id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    let board_to_be_update_status = sqlx::query(
        &format!("
        SELECT 
            id
            FROM {}.{}
            WHERE id = $1 AND owner_id = $2 AND status_id = 0
        ", 
        APP_SCHEMA, 
        BOARDS_TABLE)
    )
        .bind(id)
        .bind(user_id)
        .fetch_one(db_link)
        .await;

    match board_to_be_update_status {
        Ok(_) => {

            let query = format!("
                UPDATE {}.{}
                    SET title = $2, description = $3
                    WHERE id = $1 
                    AND owner_id = $4 
                    AND status_id = 0
                ", 
                APP_SCHEMA, 
                BOARDS_TABLE
            );
            let result = sqlx::query(&query)
                .bind(id)
                .bind(title)
                .bind(description)
                .bind(user_id)
                .execute(db_link)
                .await;

            drop_user_boards_from_redis(redis_conn, user_id);
            match result {
                Ok(_) => {
                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        HttpResponse::Ok().cookie(cookie).json(ServerResponse {
                            status: 200, 
                            message: String::from("Board updated")
                        })
                    } else {
                        HttpResponse::Ok().json(ServerResponse {
                            status: 200, 
                            message: String::from("Board updated")
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
        }, 
        Err(_) => {
            HttpResponse::BadRequest().json(ServerResponse {
                status: 400, 
                message: String::from("Invalid request")
            })
        }
    }
    
}

async fn handle_delete_board(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    board_data: Json<DeleteBoardBody>) -> impl Responder {

    let DeleteBoardBody {id} = board_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    log::info!("User {} tried to delete board {}", user_id, id);

    let board_is_valid_check = sqlx::query(
        &format!("
        SELECT 
            id
            FROM {}.{}
            WHERE id = $1 AND owner_id = $2 AND status_id = 0
        ", 
        APP_SCHEMA, 
        BOARDS_TABLE)
    )   
        .bind(id)
        .bind(user_id)
        .fetch_one(db_link)
        .await;

    match board_is_valid_check {
        Ok(_) => {

            let query = format!("
                UPDATE {}.{}
                    SET status_id = 1
                    WHERE id = $1 
                    AND owner_id = $2 
                    AND status_id = 0
                ", 
                APP_SCHEMA, 
                BOARDS_TABLE
            );
            let result = sqlx::query(&query)
                .bind(id)
                .bind(user_id)
                .execute(db_link)
                .await;
            
            drop_user_boards_from_redis(redis_conn, user_id);
            match result {
                Ok(_) => {
                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        HttpResponse::Ok().cookie(cookie).json(ServerResponse {
                            status: 200, 
                            message: String::from("Board deleted")
                        })
                    } else {
                        HttpResponse::Ok().json(ServerResponse {
                            status: 200, 
                            message: String::from("Board deleted")
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
        }, 
        Err(_) => {
            HttpResponse::BadRequest().json(ServerResponse {
                status: 400, 
                message: String::from("Invalid request")
            })
        }
    }
}
