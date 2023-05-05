use std::fs;
use actix::fut::future::result;
use actix_web::{get, post, web::{self, Data, Json}, Responder, HttpResponse};
use serde::{Serialize, Deserialize};
use sqlx;
use crate::{authorisation::{UserWebData, check_token}};
use crate::{PostgresDB, RedisDB, Logger};
use crate::models::{
    Board, StoredBoard, CreateBoardBody, UpdateBoardBody, DeleteBoardBody, 
    Task, StoredTask, GetTasksBody, CreateTaskBody, UpdateTaskBody, DeleteTaskBody
};
use chrono::{NaiveDateTime};

#[derive(Serialize)]
pub struct ServerResponse {
    status: i32, 
    message: String
}

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
    logger: Data<Logger>, 
    user_data: Json<UserWebData>) -> impl Responder {

    let user_id = user_data.0.id();
    let token = user_data.0.token();

    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let db_link = &*postgres_db.db.lock().unwrap();
        let result = sqlx::query_as!(
            StoredBoard,
            "SELECT 
                id, title, description, creation_time    
            FROM routine_app.board 
            WHERE status_id = 0 AND owner_id = $1", 
            user_id
        ).fetch_all(db_link)
            .await;

        match result {
            Ok(stored_boards_list) => {
                
                let mut board_list: Vec<Board> = vec![];
                for stored_board in stored_boards_list.iter() {
                    let board_to_return = stored_board.get_board();
                    board_list.push(board_to_return);
                }
                HttpResponse::Ok().json(board_list)
            }, 
            Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                status: 500, 
                message: format!("{}", db_error)
            })
        }

        // let current_time = chrono::offset::Utc::now();
        // let naive_date_time = chrono::Utc::now().naive_utc();
        // println!("{:?}", naive_date_time);
        // println!("{:?}", current_time);
        // println!("{:?}", current_time.timestamp()); 
        // HttpResponse::Ok().json(result)

    } else {
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }

    
}

async fn create_new_board(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    logger: Data<Logger>, 
    board_data: Json<CreateBoardBody>) -> impl Responder {

    let CreateBoardBody{user_data, title, description} = board_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();

    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let db_link = &*postgres_db.db.lock().unwrap();
        let result = sqlx::query!(
            "INSERT INTO routine_app.board (title, description, status_id, owner_id, creation_time) 
            VALUES ($1, $2, $3, $4, $5)", 
            title, 
            description, 
            0, 
            user_id, 
            current_time 
        )
            .execute(db_link)
            .await;
        match result {
            Ok(_) => HttpResponse::Ok().json(ServerResponse {
                status: 200, 
                message: String::from("Board created")
            }), 
            Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                status: 500, 
                message: format!("{}", db_error)
            })
        }
    } else {
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
    
}

async fn update_existed_board(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    logger: Data<Logger>, 
    board_data: Json<UpdateBoardBody>) -> impl Responder {

    let UpdateBoardBody{user_data, id, title, description} = board_data.0;
    let user_id = user_data.id();
    let token = user_data.token();

    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let db_link = &*postgres_db.db.lock().unwrap();
        let board_to_be_update_status = sqlx::query!(
            "SELECT 
                id
            FROM routine_app.board
            WHERE id = $1 AND owner_id = $2 AND status_id = 0",
            id, 
            user_id
        )
            .fetch_one(db_link)
            .await;

        match board_to_be_update_status {
            Ok(_) => {
                let result = sqlx::query!(
                    "UPDATE routine_app.board
                    SET title = $2, description = $3
                    WHERE id = $1 AND owner_id = $4 AND status_id = 0",
                    id, 
                    title, 
                    description, 
                    user_id
                )
                    .execute(db_link)
                    .await;
        
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Board updated")
                    }), 
                    Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                        status: 500, 
                        message: format!("{}", db_error)
                    })
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
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
    
}

async fn delete_board(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    logger: Data<Logger>, 
    board_data: Json<DeleteBoardBody>) -> impl Responder {

    let DeleteBoardBody {user_data, id} = board_data.0;
    let user_id = user_data.id();
    let token = user_data.token();

    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    )  {
        let db_link = &*postgres_db.db.lock().unwrap();
        let board_is_valid_check = sqlx::query!(
            "SELECT 
                id
            FROM routine_app.board 
            WHERE id = $1 AND owner_id = $2 AND status_id = 0", 
            id, 
            user_id
        )
            .fetch_one(db_link)
            .await;

        match board_is_valid_check {
            Ok(_) => {
                let result = sqlx::query!(
                    "UPDATE routine_app.board
                    SET status_id = 1
                    WHERE id = $1 AND owner_id = $2 AND status_id = 0",
                    id, 
                    user_id
                )
                    .execute(db_link)
                    .await;
        
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Board deleted")
                    }), 
                    Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                        status: 500, 
                        message: format!("{}", db_error)
                    })
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
    logger: Data<Logger>, 
    request_data: Json<GetTasksBody>) -> impl Responder {

    let GetTasksBody {user_data, board_id} = request_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    
    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let db_link = &*postgres_db.db.lock().unwrap();
        let is_valid_board = sqlx::query!(
            "SELECT 
                id
            FROM routine_app.board 
            WHERE id = $1 AND owner_id = $2 AND status_id = 0", 
            board_id, 
            user_id
        ).fetch_one(db_link)
            .await;

        match is_valid_board {
            Ok(_) => {
                let result = sqlx::query_as!(
                    StoredTask,
                    "   SELECT 
                            t.id, t.title, t.description, t.board_id, t.status_id, t.last_status_change_time
                          FROM routine_app.task t
                    INNER JOIN routine_app.board b
                            ON t.board_id = b.id
                         WHERE t.status_id != 4 
                           AND t.board_id = $1 AND b.owner_id = $2", 
                    board_id, 
                    user_id
                ).fetch_all(db_link)
                    .await;
        
                match result {
                    Ok(stored_task_list) => {
                        
                        let mut tasks_list: Vec<Task> = vec![];
                        for stored_task in stored_task_list.iter() {
                            let task_to_return = stored_task.get_task();
                            tasks_list.push(task_to_return);
                        }
                        HttpResponse::Ok().json(tasks_list)
                    }, 
                    Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                        status: 500, 
                        message: format!("{}", db_error)
                    })
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
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }

}

async fn create_new_task(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    logger: Data<Logger>, 
    task_data: Json<CreateTaskBody>) -> impl Responder {

    let CreateTaskBody { user_data, board_id, title, description } = task_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();
    
    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let db_link = &*postgres_db.db.lock().unwrap();
        let is_valid_board = sqlx::query!(
            "SELECT 
                id
            FROM routine_app.board 
            WHERE id = $1 AND owner_id = $2 AND status_id = 0", 
            board_id, 
            user_id
        ).fetch_one(db_link)
            .await;

        match is_valid_board {
            Ok(_) => {
                let result = sqlx::query!(
                    "INSERT INTO routine_app.task 
                        (title, description, board_id, status_id, last_status_change_time, creation_time) 
                    VALUES ($1, $2, $3, $4, $5, $5)", 
                    title, 
                    description, 
                    board_id,
                    0, 
                    current_time 
                )
                    .execute(db_link)
                    .await;
        
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Task created")
                    }), 
                    Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                        status: 500, 
                        message: format!("{}", db_error)
                    })
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
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
}

async fn update_existed_task(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    logger: Data<Logger>, 
    task_data: Json<UpdateTaskBody>) -> impl Responder {

    let UpdateTaskBody { 
        user_data, id, board_id, title, description, status_id 
    } = task_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();

    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let db_link = &*postgres_db.db.lock().unwrap();
        let task_to_be_update_status = sqlx::query!(
            "   SELECT 
                    t.status_id
                  FROM routine_app.task t
            INNER JOIN routine_app.board b
                    ON t.board_id = b.id
                 WHERE t.status_id != 4 
                   AND t.id = $1
                   AND t.board_id = $2 
                   AND b.owner_id = $3", 
            id,
            board_id, 
            user_id
        )
            .fetch_one(db_link)
            .await;

        match task_to_be_update_status {
            Ok(record) => {
                let result = if record.status_id.unwrap() == status_id { // status wasn't changed
                    sqlx::query!(
                        "UPDATE routine_app.task t
                            SET title = $2, description = $3
                        FROM routine_app.board b
                        WHERE b.id = t.board_id 
                            AND t.id = $1 
                            AND t.board_id = $4 
                            AND b.owner_id = $5 
                            AND b.status_id = 0 
                            AND t.status_id != 4",
                        id, 
                        title, 
                        description, 
                        board_id, 
                        user_id
                    )
                        .execute(db_link)
                        .await

                } else { // status was changed
                    sqlx::query!(
                        "UPDATE routine_app.task t
                            SET title = $2, description = $3, status_id = $5, last_status_change_time = $6
                        FROM routine_app.board b
                        WHERE b.id = t.board_id 
                            AND t.id = $1 
                            AND t.board_id = $4 
                            AND b.owner_id = $7 
                            AND b.status_id = 0 
                            AND t.status_id != 4",
                        id, 
                        title, 
                        description, 
                        board_id, 
                        status_id,
                        current_time,
                        user_id
                    )
                        .execute(db_link)
                        .await
                };
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Task updated")
                    }), 
                    Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                        status: 500, 
                        message: format!("{}", db_error)
                    })
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
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
}

async fn delete_task(
    postgres_db: Data<PostgresDB>, 
    redis_db: Data<RedisDB>, 
    logger: Data<Logger>, 
    task_data: Json<DeleteTaskBody>) -> impl Responder {

    let DeleteTaskBody { user_data, id, board_id } = task_data.0;
    let user_id = user_data.id();
    let token = user_data.token();
    let current_time = chrono::offset::Utc::now().naive_utc();

    if check_token(
        UserWebData {user_id, token: token.clone()}, 
        UserWebData {user_id, token: token.clone()}
    ) {
        let db_link = &*postgres_db.db.lock().unwrap();
        let task_is_valid_check = sqlx::query!(
            "   SELECT 
                    t.id
                  FROM routine_app.task t
            INNER JOIN routine_app.board b
                    ON t.board_id = b.id
                 WHERE t.status_id != 4 
                   AND t.id = $1
                   AND t.board_id = $2 
                   AND b.owner_id = $3", 
            id,
            board_id, 
            user_id
        )
            .fetch_one(db_link)
            .await;

        match task_is_valid_check {
            Ok(_) => {
                let result = sqlx::query!(
                    "UPDATE routine_app.task t
                        SET status_id = 4, last_status_change_time = $4
                       FROM routine_app.board b
                      WHERE b.id = t.board_id 
                        AND t.id = $1 
                        AND t.board_id = $2 
                        AND b.owner_id = $3 
                        AND b.status_id = 0
                        AND t.status_id != 4",
                    id, 
                    board_id, 
                    user_id, 
                    current_time
                )
                    .execute(db_link)
                    .await;
        
                match result {
                    Ok(_) => HttpResponse::Ok().json(ServerResponse {
                        status: 200, 
                        message: String::from("Task deleted")
                    }), 
                    Err(db_error) => HttpResponse::InternalServerError().json(ServerResponse {
                        status: 500, 
                        message: format!("{}", db_error)
                    })
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
        HttpResponse::BadRequest().json(ServerResponse {
            status: 400, 
            message: String::from("Invalid user credentials")
        })
    }
    
    
}