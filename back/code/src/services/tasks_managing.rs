use actix_web::{
    web::{self, Data, Json}, 
    Responder, HttpRequest, HttpResponse, 
    cookie::{time::Duration, Cookie}
};
use log;
use sqlx::{self, Row};
use uuid::Uuid;

use crate::{PersistentDB, CacheDB, APP_SCHEMA, BOARDS_TABLE, TASKS_TABLE, TOKEN_LIFETIME, models::Board};
use crate::redis_handlers::{
    get_board_tasks_from_redis, 
    put_board_tasks_to_redis, 
    drop_board_tasks_from_redis, 
    get_user_boards_from_redis
};
use crate::models::{
    ServerResponse, Task, StoredTask, GetTasksBody, 
    CreateTaskBody, UpdateTaskBody, DeleteTaskBody
};

pub fn tasks_managing(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::resource("/board_tasks")
                .route(web::get().to(handle_board_tasks))
        ).service(
            web::resource("/task/{task_id}")
                .route(web::get().to(handle_task))
        )
        .service(
            web::resource("/create_task")
                .route(web::post().to(handle_create_task))
        )
        .service(
            web::resource("/change_task")
                .route(web::put().to(handle_change_task))
        )
        .service(
            web::resource("/delete_task")
                .route(web::delete().to(handle_delete_task))
        );
}

async fn handle_board_tasks(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    request_data: Json<GetTasksBody>) -> impl Responder {

    let GetTasksBody {board_id} = request_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    log::info!("Tasks from board {} requested by user {}", board_id, user_id);

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
                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        return HttpResponse::Ok().cookie(cookie).json(redis_tasks_list);
                    } else {
                        return HttpResponse::Ok().json(redis_tasks_list);
                    }
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

                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        HttpResponse::Ok().cookie(cookie).json(tasks_list)
                    } else {
                        HttpResponse::Ok().json(tasks_list)
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

async fn handle_task(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>,
    request_path: web::Path<i32>) -> impl Responder {

    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();
    let board_id: i32 = headers.get("BoardId").unwrap().to_str().unwrap().parse().unwrap();
    let task_id = request_path.into_inner();

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

    if let Ok(user_boards) = get_user_boards_from_redis(redis_conn, user_id) {
        let mut target_board: Option<Board> = None;
        for board in user_boards {
            if board.id == board_id {
                target_board = Some(board);
                break;
            }
        }
        match target_board {
            Some(_) => {
                if let Ok(tasks) = get_board_tasks_from_redis(redis_conn, board_id) {
                    for task in tasks {
                        if task.id == task_id {
                            if headers.contains_key("new_token") {
                                let token = headers.get("new_token").unwrap().to_str().unwrap();
                                let mut cookie = Cookie::new("x-auth", token);
                                cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));
        
                                return HttpResponse::Ok().cookie(cookie).json(task);
                            } else {
                                return HttpResponse::Ok().json(task);
                            }
                        }
                    }
                }
            }, 
            None => ()
            }
        }

    let is_valid_task = sqlx::query(
        &format!("
            SELECT 
                t.id
            FROM {APP_SCHEMA}.{TASKS_TABLE} t
            INNER JOIN {APP_SCHEMA}.{BOARDS_TABLE} b
            ON t.board_id = b.id
            WHERE t.id = $1 
            AND t.board_id = $2
            AND b.owner_id = $3 
            AND t.status_id != 4
            AND b.status_id = 0"
        )
    )
        .bind(task_id)
        .bind(board_id)
        .bind(user_id)
        .fetch_one(db_link)
        .await;

    match is_valid_task {
        Ok(_) => {
            let query = format!("
                SELECT 
                    t.id, t.title, t.description, t.board_id, t.status_id, t.last_status_change_time
                FROM {APP_SCHEMA}.{TASKS_TABLE} t
                WHERE t.status_id != 4 
                AND t.board_id = $1 
            ");
            let result = sqlx::query(&query)
                .bind(board_id)
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
                Ok(tasks) => {
                    let target_task = tasks[0].get_task();

                    put_board_tasks_to_redis(redis_conn, board_id, &[target_task.clone()]);

                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        HttpResponse::Ok().cookie(cookie).json(target_task)
                    } else {
                        HttpResponse::Ok().json(target_task)
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
            log::warn!("User {} tried to request non-matching values: task {} from board {}", user_id, task_id, board_id);
            HttpResponse::BadRequest().json(ServerResponse {
                status: 400, 
                message: String::from("Invalid request")
            })
        }
    }
}

async fn handle_create_task(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    task_data: Json<CreateTaskBody>) -> impl Responder {

    let CreateTaskBody {board_id, title, description } = task_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    log::info!("User {} tried to create new task on board {}", user_id, board_id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();
    
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
                    (title, description, board_id, status_id) 
                VALUES ($1, $2, $3, 0)", 
                APP_SCHEMA, 
                TASKS_TABLE
            );
            let result = sqlx::query(&query)
                .bind(title)
                .bind(description)
                .bind(board_id)
                .execute(db_link)
                .await;

            drop_board_tasks_from_redis(redis_conn, board_id);
            match result {
                Ok(_) => {
                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        HttpResponse::Ok().cookie(cookie).json(ServerResponse {
                            status: 200, 
                            message: String::from("Task created")
                        })
                    } else {
                        HttpResponse::Ok().json(ServerResponse {
                            status: 200, 
                            message: String::from("Task created")
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

async fn handle_change_task(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    task_data: Json<UpdateTaskBody>) -> impl Responder {

    let UpdateTaskBody { 
        id, board_id, title, description, status_id 
    } = task_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    log::info!("User {} tried to change task {}", user_id, id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

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
                        SET title = $2, description = $3, status_id = $6
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
                    .execute(db_link)
                    .await
            };

            drop_board_tasks_from_redis(redis_conn, board_id);
            match result {
                Ok(_) => {
                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        HttpResponse::Ok().cookie(cookie).json(ServerResponse {
                            status: 200, 
                            message: String::from("Task updated")
                        })
                    } else {
                        HttpResponse::Ok().json(ServerResponse {
                            status: 200, 
                            message: String::from("Task updated")
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

async fn handle_delete_task(
    request: HttpRequest,
    postgres_db: Data<PersistentDB>, 
    redis_db: Data<CacheDB>, 
    task_data: Json<DeleteTaskBody>) -> impl Responder {

    let DeleteTaskBody {id, board_id } = task_data.0;
    let headers = request.headers();
    let user_id: Uuid = headers.get("user_id").unwrap().to_str().unwrap().parse().unwrap();

    log::info!("User {} tried to delete task {}", user_id, id);

    let db_link = &*postgres_db.db.lock().unwrap();
    let redis_conn = &mut *redis_db.db.lock().unwrap();

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
                    SET status_id = 4
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
                .execute(db_link)
                .await;

            drop_board_tasks_from_redis(redis_conn, board_id);
            match result {
                Ok(_) => {
                    if headers.contains_key("new_token") {
                        let token = headers.get("new_token").unwrap().to_str().unwrap();
                        let mut cookie = Cookie::new("x-auth", token);
                        cookie.set_max_age(Duration::seconds(TOKEN_LIFETIME));

                        HttpResponse::Ok().cookie(cookie).json(ServerResponse {
                            status: 200, 
                            message: String::from("Task deleted")
                        })
                    } else {
                        HttpResponse::Ok().json(ServerResponse {
                            status: 200, 
                            message: String::from("Task deleted")
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
