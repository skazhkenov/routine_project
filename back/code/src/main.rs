use std::sync::Mutex;
use actix_web::{web, App, HttpServer};
use sqlx::{postgres::{PgPoolOptions, self}, Postgres, Pool};
use redis::{self, Commands, Connection as RedisConnection};

mod authorisation;
mod logging;
mod models;
mod services;

use services::{boards_managing, tasks_managing};
use logging::LogWriter;

const HOST: &'static str = "127.0.0.1:5000";
const REDIS_HOST: &'static str = "redis://192.168.0.103:4444";
const LOG_FILE_PATH: &'static str = "./logs/log";
pub const STATIC_FILES_PATH: &'static str = "./static_files";
pub const START_PAGE_FILE_NAME: &'static str = "index.html";
pub const NOT_FOUND_FILE_NAME: &'static str = "404.html";

// postgres data model
pub const APP_SCHEMA: &'static str = "routine_app";
pub const USERS_TABLE: &'static str = "customer";
pub const BOARDS_TABLE: &'static str = "board";
pub const TASKS_TABLE: &'static str = "task";


pub struct PostgresDB {
    db: Mutex<Pool<Postgres>>
}

pub struct RedisDB {
    db: Mutex<RedisConnection>
}
pub struct Logger {
    file_path: &'static str, 
    logger: Mutex<LogWriter>
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    
    dotenv::dotenv().expect("Unable to load environment variables from .env file");
    let db_url = std::env::var("DATABASE_URL").expect("Unable to read DATABASE_URL env var");

    logging::log("Service started", Ok("Ok"));
    

    let postgres_pool = PgPoolOptions::new()
        .max_connections(50)
        .connect(&db_url)
        .await
        .expect("Unable to connect to Postgres");
    
    let redis_client = redis::Client::open(REDIS_HOST).unwrap();
    let redis_connection = redis_client.get_connection().unwrap();

    let postgres_db = web::Data::new(
        PostgresDB {
            db: Mutex::new(postgres_pool)
        }
    );
    let redis_db = web::Data::new(
        RedisDB {
            db: Mutex::new(redis_connection)
        }
    );
    let logger = web::Data::new(
        Logger {
            file_path: LOG_FILE_PATH, 
            logger: Mutex::new(LogWriter)
        }
    );

    println!("Application start");

    HttpServer::new(move || {
        App::new()
            .app_data(postgres_db.clone())
            .app_data(redis_db.clone())
            .app_data(logger.clone())
            .configure(boards_managing)
            .configure(tasks_managing)
    })
        .bind(HOST)?
        .workers(3)
        .run()
        .await

}
