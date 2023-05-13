use actix::fut::future::result;
use actix_web::{web::{self, Data, Json}, Responder, HttpResponse};
use serde::{Serialize, Deserialize};
use sqlx::{self, Row};

use redis::{Commands};
use chrono::{NaiveDateTime};
use log;

use crate::{authorisation::{UserWebData, is_valid_token}};
use crate::{PostgresDB, RedisDB};
use crate::redis_handlers::{
    get_user_boards_from_redis, put_user_boards_to_redis, drop_user_boards_from_redis, 
    get_board_tasks_from_redis, put_board_tasks_to_redis, drop_board_tasks_from_redis
};
use crate::models::{
    ServerResponse, Board, StoredBoard, CreateBoardBody, UpdateBoardBody, DeleteBoardBody, 
    Task, StoredTask, GetTasksBody, CreateTaskBody, UpdateTaskBody, DeleteTaskBody
};
use crate::{APP_SCHEMA, BOARDS_TABLE, TASKS_TABLE};

// Boards section

pub fn boards_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/user_boards")
                .route(web::get().to(get_user_boards))
        )
        .service(
            web::resource("/create_board")
                .route(web::post().to(create_new_board))
        )
        .service(
            web::resource("/change_board")
                .route(web::put().to(update_existed_board))
        )
        .service(
            web::resource("/delete_board")
                .route(web::delete().to(delete_board))
        );
}

async fn get_user_boards(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    user_data: Json<UserWebData>) -> impl Responder {

    let user_id = user_data.0.id();
    let token = user_data.0.token();

    log::info!("Boards requested by user {}", user_id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let redis_data = get_user_boards_from_redis(redis_conn, user_id);
        if let Ok(redis_boards_list) = redis_data {
            if redis_boards_list.len() > 0 {
                return HttpResponse::Ok().json(redis_boards_list);
            }
        }

        let query = format!(
            "SELECT 
                id, title, description, creation_time    
               FROM {}.{}
              WHERE status_id = 0 AND owner_id = $1", 
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
                HttpResponse::Ok().json(board_list)
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

async fn create_new_board(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    board_data: Json<CreateBoardBody>) -> impl Responder {

    let CreateBoardBody{user_data, title, description} = board_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();

    log::info!("Creation new board by user {}, at {}", user_id, current_time);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let query = format!(
            "INSERT INTO {}.{} (title, description, status_id, owner_id, creation_time) 
                  VALUES ($1, $2, $3, $4, $5)", 
            APP_SCHEMA, 
            BOARDS_TABLE
        );
        let result = sqlx::query(&query)
            .bind(title)
            .bind(description)
            .bind(0)
            .bind(user_id)
            .bind(current_time)
            .execute(db_link)
            .await;

        drop_user_boards_from_redis(redis_conn, user_id);
        match result {
            Ok(_) => {
                HttpResponse::Ok().json(ServerResponse {
                    status: 200, 
                    message: String::from("Board created")
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

async fn update_existed_board(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    board_data: Json<UpdateBoardBody>) -> impl Responder {

    let UpdateBoardBody{user_data, id, title, description} = board_data.0;
    let user_id = user_data.id();
    let token = user_data.token();

    log::info!("User {} tried to change board {}", user_id, id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
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
                        HttpResponse::Ok().json(ServerResponse {
                            status: 200, 
                            message: String::from("Board updated")
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
            }, 
            Err(_) => {
                HttpResponse::BadRequest().json(ServerResponse {
                    status: 400, 
                    message: String::from("Invalid request")
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

async fn delete_board(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    board_data: Json<DeleteBoardBody>) -> impl Responder {

    let DeleteBoardBody {user_data, id} = board_data.0;
    let user_id = user_data.id();
    let token = user_data.token();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    log::info!("User {} tried to delete board {}", user_id, id);

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    )  {
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
                        HttpResponse::Ok().json(ServerResponse {
                            status: 200, 
                            message: String::from("Board deleted")
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
            }, 
            Err(_) => {
                HttpResponse::BadRequest().json(ServerResponse {
                    status: 400, 
                    message: String::from("Invalid request")
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


// Tasks section

pub fn tasks_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/board_tasks")
                .route(web::get().to(get_board_tasks))
        )
        .service(
            web::resource("/create_task")
                .route(web::post().to(create_new_task))
        )
        .service(
            web::resource("/change_task")
                .route(web::put().to(update_existed_task))
        )
        .service(
            web::resource("/delete_task")
                .route(web::delete().to(delete_task))
        );
}

async fn get_board_tasks(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    request_data: Json<GetTasksBody>) -> impl Responder {

    let GetTasksBody {user_data, board_id} = request_data.0;
    let user_id = user_data.id();
    let token = user_data.token();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    log::info!("Tasks from board {} requested by user {}", board_id, user_id);
    
    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {

        let is_valid_board = sqlx::query(
            &format!("
                SELECT 
                    id
                  FROM {}.{} 
                 WHERE id = $1 
                   AND owner_id = $2 
                   AND status_id = 0", 
                APP_SCHEMA, 
                BOARDS_TABLE
            )
        )
            .bind(board_id)
            .bind(user_id)
            .fetch_one(db_link)
            .await;

        match is_valid_board {
            Ok(_) => {
                let redis_data = get_board_tasks_from_redis(redis_conn, board_id);
                if let Ok(redis_tasks_list) = redis_data {
                    if redis_tasks_list.len() > 0 {
                        return HttpResponse::Ok().json(redis_tasks_list);
                    }
                }

                let query = format!("
                    SELECT 
                        t.id, t.title, t.description, t.board_id, t.status_id, t.last_status_change_time
                      FROM {APP_SCHEMA}.{TASKS_TABLE} t
                INNER JOIN {APP_SCHEMA}.{BOARDS_TABLE} b
                        ON t.board_id = b.id
                     WHERE t.status_id != 4 
                       AND t.board_id = $1 
                       AND b.owner_id = $2
                ");
                let result = sqlx::query(&query)
                    .bind(board_id)
                    .bind(user_id)
                    .map(|row| {
                        StoredTask {
                            id: row.get("id"),
                            title: row.get("title"),
                            description: row.get("description"),
                            board_id: row.get("board_id"), 
                            status_id: row.get("status_id"), 
                            last_status_change_time: row.get("last_status_change_time")
                        } 
                    })
                    .fetch_all(db_link)
                    .await; 

                match result {
                    Ok(stored_task_list) => {
                        
                        let mut tasks_list: Vec<Task> = vec![];
                        for stored_task in stored_task_list.iter() {
                            let task_to_return = stored_task.get_task();
                            tasks_list.push(task_to_return);
                        }
                        put_board_tasks_to_redis(redis_conn, board_id, &tasks_list);
                        HttpResponse::Ok().json(tasks_list)
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
        
    } else {
        log::warn!("Unauthorised user {} tried to get access", user_id);
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }

}

async fn create_new_task(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    task_data: Json<CreateTaskBody>) -> impl Responder {

    let CreateTaskBody { user_data, board_id, title, description } = task_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();

    log::info!("User {} tried to create new task on board {} at {}", user_id, board_id, current_time);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();
    
    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let is_valid_board = sqlx::query(
            &format!("
                SELECT 
                    id
                  FROM {}.{} 
                 WHERE id = $1 
                   AND owner_id = $2 
                   AND status_id = 0", 
                APP_SCHEMA, 
                BOARDS_TABLE
            )
        )
            .bind(board_id)
            .bind(user_id)
            .fetch_one(db_link)
            .await;
        
        match is_valid_board {
            Ok(_) => {
                let query = format!("
                    INSERT INTO {}.{} 
                        (title, description, board_id, status_id, last_status_change_time, creation_time) 
                    VALUES ($1, $2, $3, $4, $5, $5)", 
                    APP_SCHEMA, 
                    TASKS_TABLE
                );
                let result = sqlx::query(&query)
                    .bind(title)
                    .bind(description)
                    .bind(board_id)
                    .bind(0)
                    .bind(current_time)
                    .execute(db_link)
                    .await;

                drop_board_tasks_from_redis(redis_conn, board_id);
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Task created")
                    }), 
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

    } else {
        log::warn!("Unauthorised user {} tried to get access", user_id);
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
}

async fn update_existed_task(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    task_data: Json<UpdateTaskBody>) -> impl Responder {

    let UpdateTaskBody { 
        user_data, id, board_id, title, description, status_id 
    } = task_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();

    log::info!("User {} tried to change task {} at {}", user_id, id, current_time);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let task_to_be_update_status = sqlx::query(
            &format!("
                SELECT 
                    t.status_id
                  FROM {APP_SCHEMA}.{TASKS_TABLE} t
            INNER JOIN {APP_SCHEMA}.{BOARDS_TABLE} b
                    ON t.board_id = b.id
                 WHERE t.status_id != 4 
                   AND t.id = $1
                   AND t.board_id = $2 
                   AND b.owner_id = $3")
        )
            .bind(id)
            .bind(board_id)
            .bind(user_id)
            .fetch_one(db_link)
            .await;

        match task_to_be_update_status {
            Ok(record) => {
                let result = if record.get::<i32, &str>("status_id") == status_id { // status wasn't changed
                    let query = format!("
                        UPDATE {APP_SCHEMA}.{TASKS_TABLE} t
                           SET title = $2, description = $3
                          FROM {APP_SCHEMA}.{BOARDS_TABLE} b
                         WHERE b.id = t.board_id 
                           AND t.id = $1 
                           AND t.board_id = $4 
                           AND b.owner_id = $5 
                           AND b.status_id = 0 
                           AND t.status_id != 4"
                    );
                    sqlx::query(&query)
                        .bind(id)
                        .bind(title)
                        .bind(description)
                        .bind(board_id)
                        .bind(user_id)
                        .execute(db_link)
                        .await

                } else { // status was changed
                    let query = format!("
                        UPDATE {APP_SCHEMA}.{TASKS_TABLE} t
                           SET title = $2, description = $3, status_id = $6, last_status_change_time = $7
                          FROM {APP_SCHEMA}.{BOARDS_TABLE} b
                         WHERE b.id = t.board_id 
                           AND t.id = $1 
                           AND t.board_id = $4 
                           AND b.owner_id = $5 
                           AND b.status_id = 0 
                           AND t.status_id != 4"
                    );
                    sqlx::query(&query)
                        .bind(id)
                        .bind(title)
                        .bind(description)
                        .bind(board_id)
                        .bind(user_id)
                        .bind(status_id)
                        .bind(current_time)
                        .execute(db_link)
                        .await
                };

                drop_board_tasks_from_redis(redis_conn, board_id);
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Task updated")
                    }), 
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

    } else {
        log::warn!("Unauthorised user {} tried to get access", user_id);
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
}

async fn delete_task(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    task_data: Json<DeleteTaskBody>) -> impl Responder {

    let DeleteTaskBody { user_data, id, board_id } = task_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();

    log::info!("User {} tried to delete task {} at {}", user_id, id, current_time);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if is_valid_token(
        redis_conn, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let task_is_valid_check = sqlx::query(
            &format!("   
                SELECT 
                    t.id
                  FROM {APP_SCHEMA}.{TASKS_TABLE} t
            INNER JOIN {APP_SCHEMA}.{BOARDS_TABLE} b
                    ON t.board_id = b.id
                 WHERE t.status_id != 4 
                   AND t.id = $1
                   AND t.board_id = $2 
                   AND b.owner_id = $3")
        )
            .bind(id)
            .bind(board_id)
            .bind(user_id)
            .fetch_one(db_link)
            .await;

        match task_is_valid_check {
            Ok(_) => {
                let query = format!("
                    UPDATE {APP_SCHEMA}.{TASKS_TABLE} t
                       SET status_id = 4, last_status_change_time = $4
                      FROM {APP_SCHEMA}.{BOARDS_TABLE} b
                     WHERE b.id = t.board_id 
                       AND t.id = $1 
                       AND t.board_id = $2 
                       AND b.owner_id = $3 
                       AND b.status_id = 0
                       AND t.status_id != 4"
                ); 
                let result = sqlx::query(&query)
                    .bind(id)
                    .bind(board_id)
                    .bind(user_id)
                    .bind(current_time)
                    .execute(db_link)
                    .await;

                drop_board_tasks_from_redis(redis_conn, board_id);
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Task deleted")
                    }), 
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
    } else {
        log::warn!("Unauthorised user {} tried to get access", user_id);
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
    
}

