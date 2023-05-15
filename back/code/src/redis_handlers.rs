use redis::{Commands, RedisResult, RedisError};
use crate::models::{User, Board, Task};
use serde_json;
use uuid::Uuid;

use crate::{STORED_DATA_EXPIRATION_TIME, USER_DATA_EXPIRATION_TIME};

// User handlers

pub fn set_email_userid_map_to_redis(
    conn: &mut redis::Connection, 
    email: String, 
    user_id: Uuid) -> RedisResult<()> {
    let key = format!("user_email:{}:id", email);

    conn.set(&key, user_id.to_string())?;
    conn.expire(&key, USER_DATA_EXPIRATION_TIME)?;

    Ok(())
}

pub fn check_email_in_redis(
    conn: &mut redis::Connection, 
    email: &str) -> RedisResult<Uuid> {
    let key = format!("user_email:{}:id", email);
    
    let user_id_from_redis: String = conn.get(key)?; 
    match Uuid::parse_str(&user_id_from_redis) {
        Ok(user_id) => {
            Ok(user_id)
        }, 
        Err(_) => {
            let error_message: &'static str = "Redis db error";
            Err(RedisError::from(std::io::Error::new(std::io::ErrorKind::Other, error_message)))
        }
    }
}

pub fn put_user_data_to_redis(
    conn: &mut redis::Connection, 
    user: User, 
    lifetime: Option<usize>) -> RedisResult<()> {
    let key = format!("user_id:{}:data", user.id);

    let json = serde_json::to_string(&user).unwrap();
    conn.set(&key, json)?;
    match lifetime {
        Some(custom_time) => {
            conn.expire(&key, custom_time)?;
        }, 
        None => {
            conn.expire(&key, USER_DATA_EXPIRATION_TIME)?;
        }
    }
    
    set_email_userid_map_to_redis(conn, user.email.clone(), user.id)?;

    Ok(())
}

pub fn get_user_data_by_email_from_redis(
    conn: &mut redis::Connection, 
    email: &str) -> RedisResult<User> {
    let user_id = check_email_in_redis(conn, email)?;
    let user = get_user_data_by_id_from_redis(conn, user_id)?;

    Ok(user)
}

pub fn get_user_data_by_id_from_redis(
    conn: &mut redis::Connection, 
    user_id: Uuid) -> RedisResult<User> {
    let key = format!("user_id:{}:data", user_id);

    let user_data: String = conn.get(&key)?;
    let user: User = serde_json::from_str(&user_data).unwrap();

    Ok(user)
}

pub fn drop_user_data_from_redis(
    conn: &mut redis::Connection, 
    user_id: Uuid) {
    let key = format!("user_id:{}:data", user_id);

    conn.del::<&std::string::String, std::string::String>(&key);
}

// Board handlers

pub fn put_user_boards_to_redis(
    conn: &mut redis::Connection, 
    user_id: Uuid, 
    boards: &[Board]) -> RedisResult<()> {
    let key = format!("user:{}:boards", user_id);

    for board in boards {
        let json = serde_json::to_string(board).unwrap();
        conn.rpush(&key, json)?;
    }
    conn.expire(&key, STORED_DATA_EXPIRATION_TIME)?;
    Ok(())
}

pub fn get_user_boards_from_redis(
    conn: &mut redis::Connection, 
    user_id: Uuid) -> RedisResult<Vec<Board>> {
    let key = format!("user:{}:boards", user_id);

    let results: Vec<String> = conn.lrange(&key, 0, -1)?;

    let mut boards_list = Vec::new();
    for res in results.iter() {
        let board: Board = serde_json::from_str(res).unwrap();
        boards_list.push(board);
    }

    Ok(boards_list)
}

pub fn drop_user_boards_from_redis(
    conn: &mut redis::Connection, 
    user_id: Uuid) {
    let key = format!("user:{}:boards", user_id);
    conn.del::<&std::string::String, i32>(&key);
}

// Task handlers

pub fn put_board_tasks_to_redis(
    conn: &mut redis::Connection, 
    board_id: i32, 
    tasks: &[Task]) -> RedisResult<()> {
    let key = format!("board:{}:tasks", board_id);

    for task in tasks {
        let json = serde_json::to_string(task).unwrap();
        conn.rpush(&key, json)?;
    }
    conn.expire(&key, STORED_DATA_EXPIRATION_TIME)?;
    Ok(())
}

pub fn get_board_tasks_from_redis(
    conn: &mut redis::Connection, 
    board_id: i32) -> RedisResult<Vec<Task>> {
    let key = format!("board:{}:tasks", board_id);

    let results: Vec<String> = conn.lrange(&key, 0, -1)?;
    let mut tasks_list = Vec::new();
    for res in results.iter() {
        let task: Task = serde_json::from_str(res).unwrap();
        tasks_list.push(task);
    }

    Ok(tasks_list)
}

pub fn drop_board_tasks_from_redis(
    conn: &mut redis::Connection, 
    board_id: i32) {
    let key = format!("board:{}:tasks", board_id);
    conn.del::<&std::string::String, i32>(&key);
}