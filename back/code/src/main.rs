#[macro_use]
extern crate log;
extern crate log4rs;

use std::sync::Mutex;
use actix_web::{web, App, HttpServer};
use sqlx::{postgres::{PgPoolOptions, self}, Postgres, Pool};
use redis::{self, Commands, Connection as RedisConnection};
use log4rs::config::{self, Config};

mod authorisation;
mod models;
mod redis_handlers;
mod services;

use services::{boards_managing, tasks_managing};

const HOST: &'static str = "127.0.0.1:5000";
const REDIS_HOST: &'static str = "redis://192.168.0.103:4444";
const LOG_FILE_PATH: &'static str = "./logs/log";

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    
    dotenv::dotenv().expect("Unable to load environment variables from .env file");
    let db_url = std::env::var("DATABASE_URL").expect("Unable to read DATABASE_URL env var");

    let log_config: Config = log4rs::config::load_config_file("log_config.yml", Default::default()).unwrap();
    let log_handle: log4rs::Handle = log4rs::init_config(log_config).unwrap();

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

    log::info!("Start of application");

    HttpServer::new(move || {
        App::new()
            .app_data(postgres_db.clone())
            .app_data(redis_db.clone())
            .configure(boards_managing)
            .configure(tasks_managing)
    })
        .bind(HOST)?
        .workers(3)
        .run()
        .await

}
