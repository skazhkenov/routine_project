use std::sync::Mutex;
use sqlx::{postgres::PgPoolOptions, Postgres, Pool};
use redis::{self, Connection as RedisConnection};
use actix_web::web;

use crate::POSTGRESQL_CONNECTIONS_LIMIT;

pub struct PersistentDB {
    pub db: Mutex<Pool<Postgres>>
}

pub struct CacheDB {
    pub db: Mutex<RedisConnection>
}

pub async fn init_persistent_database() -> web::Data<PersistentDB> {

    let db_url = std::env::var("DATABASE_URL")
        .expect("Unable to read DATABASE_URL env var");
    let postgres_pool = PgPoolOptions::new()
        .max_connections(POSTGRESQL_CONNECTIONS_LIMIT)
        .connect(&db_url)
        .await
        .expect("Unable to connect to Postgres");

    let postgres_db = web::Data::new(
        PersistentDB {
            db: Mutex::new(postgres_pool)
        }
    );

    postgres_db
}

pub fn init_cache_database() -> web::Data<CacheDB> {

    let cache_db_url = std::env::var("REDIS_URL")
        .expect("Unable to read REDIS_URL env var");

    let redis_client = redis::Client::open(cache_db_url).unwrap();
    let redis_connection = redis_client.get_connection().unwrap();

    let redis_db = web::Data::new(
        CacheDB {
            db: Mutex::new(redis_connection)
        }
    );

    redis_db
}