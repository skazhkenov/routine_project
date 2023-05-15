#[macro_use]
extern crate log;
extern crate log4rs;

use actix_web::{web, App, HttpServer};
use actix_web_httpauth::middleware::HttpAuthentication;

mod autorization;
mod app_config;
mod databases;
mod logging;
mod users_managing;
mod convertations;
mod models;
mod redis_handlers;
mod services;
mod tools;

pub use app_config::*;
use autorization::validate_user;
use users_managing::{authorized_users_managing, unauthorized_users_managing};
use services::{boards_managing, tasks_managing};
use databases::{init_persistent_database, init_cache_database};
pub use databases::{PersistentDB, CacheDB};
use logging::init_logger;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    
    #[cfg(debug_assertions)]
    dotenv::dotenv().expect("Unable to load environment variables from .env file");

    init_logger();
    let postgres_db = init_persistent_database().await;
    let redis_db = init_cache_database();

    HttpServer::new(move || {
        let authorization_middleware = HttpAuthentication::bearer(validate_user);
        App::new()
            .app_data(postgres_db.clone())
            .app_data(redis_db.clone())
            .configure(unauthorized_users_managing)
            .service(
                web::scope("")
                    .wrap(authorization_middleware)
                    .configure(authorized_users_managing)
                    .configure(boards_managing)
                    .configure(tasks_managing)
            )
    })
        .bind(HOST)?
        .workers(THREADS_COUNT)
        .run()
        .await

}
