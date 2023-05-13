use redis::{Commands, RedisResult};
use crate::models::{User, Board, Task};
use serde_json;

const STORED_DATA_EXPIRATION_TIME: usize = 600; // 10 min cache lifetime
const USER_DATA_EXPIRATION_TIME: usize = 3600; // 60 min cache lifetime

// User handlers

pub fn set_email_userid_map_to_redis(conn: &mut redis::Connection, email: String, user_id: i32) -> RedisResult<()> {
    let key = format!("user_email:{}:id", email);

    conn.set(&key, user_id.to_string())?;
    conn.expire(&key, USER_DATA_EXPIRATION_TIME)?;

    Ok(())
}

pub fn check_email_in_redis(conn: &mut redis::Connection, email: &str) -> RedisResult<i32> {
    let key = format!("user_email:{}:id", email);
    let lifetime: i64 = conn.ttl(&key)?;
    if lifetime <= 0 {
        conn.del(&key)?;
    }

    let user_id: i32 = conn.get(key)?;
    Ok(user_id)
}


pub fn put_user_data_to_redis(conn: &mut redis::Connection, user: User, lifetime: Option<usize>) -> RedisResult<()> {
    let key = format!("user_id:{}:data", user.id);
    conn.del(&key)?;

    let json = serde_json::to_string(&user).unwrap();
    println!("Data stored to redis - {}", json);
    conn.hset(&key, "data", json)?;
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

pub fn get_user_data_by_email_from_redis(conn: &mut redis::Connection, email: &str) -> RedisResult<User> {
    let user_id = check_email_in_redis(conn, email)?;
    let user = get_user_data_by_id_from_redis(conn, user_id)?;

    Ok(user)
}

pub fn get_user_data_by_id_from_redis(conn: &mut redis::Connection, user_id: i32) -> RedisResult<User> {
    let key = format!("user_id:{}:data", user_id);
    let lifetime: i64 = conn.ttl(&key)?;
    if lifetime <= 0 {
        conn.del(&key)?;
    } 

    let user_data: String = conn.hget(&key, "data")?;
    let user: User = serde_json::from_str(&user_data).unwrap();

    println!("Data extracted from redis - user with id {}", user.id);

    Ok(user)
}

pub fn get_user_data_lifetime_from_redis(conn: &mut redis::Connection, user_id: i32) -> RedisResult<usize> {
    let key = format!("user_id:{}:data", user_id);
    let lifetime: i64 = conn.ttl(&key)?;
    if lifetime <= 0 {
        Ok(0)
    } else {
        Ok(lifetime as usize)
    }
}

pub fn drop_user_data_from_redis(conn: &mut redis::Connection, user_id: i32) {
    let key = format!("user_id:{}:data", user_id);
    conn.del::<&std::string::String, i32>(&key);
}


pub fn put_user_token_to_redis(conn: &mut redis::Connection, user_id: i32, token: String, lifetime: Option<usize>) -> RedisResult<()> {
    let key = format!("user_id:{}:token", user_id);
    conn.del(&key)?;

    conn.set(&key, token)?;
    match lifetime {
        Some(custom_time) => {
            conn.expire(&key, custom_time)?;
        }, 
        None => {
            conn.expire(&key, USER_DATA_EXPIRATION_TIME)?;
        }
    }

    println!("Token saved to redis for user {}", user_id);

    Ok(())
}

pub fn get_user_token_from_redis(conn: &mut redis::Connection, user_id: i32) -> RedisResult<String> {
    let key = format!("user_id:{}:token", user_id);
    let lifetime: i64 = conn.ttl(&key)?;
    if lifetime <= 0 {
        conn.del(&key)?;
    }

    let token: String = conn.get(key)?;
    println!("Token extracted from redis for user {}", user_id);
    Ok(token)
}

pub fn drop_user_token_from_redis(conn: &mut redis::Connection, user_id: i32) {
    let key = format!("user_id:{}:token", user_id);
    conn.del::<&std::string::String, i32>(&key);
}

// Board handlers

pub fn put_user_boards_to_redis(conn: &mut redis::Connection, user_id: i32, boards: &[Board]) -> RedisResult<()> {
    let key = format!("user:{}:boards", user_id);

    for board in boards {
        let json = serde_json::to_string(board).unwrap();
        conn.rpush(&key, json)?;
    }
    conn.expire(&key, STORED_DATA_EXPIRATION_TIME)?;
    Ok(())
}

pub fn get_user_boards_from_redis(conn: &mut redis::Connection, user_id: i32) -> RedisResult<Vec<Board>> {
    let key = format!("user:{}:boards", user_id);
    let lifetime: i64 = conn.ttl(&key)?;
    if lifetime <= 0 {
        conn.del(&key)?;
    }

    let results: Vec<String> = conn.lrange(&key, 0, -1)?;

    let mut boards_list = Vec::new();
    for res in results.iter() {
        let board: Board = serde_json::from_str(res).unwrap();
        boards_list.push(board);
    }

    Ok(boards_list)
}

pub fn drop_user_boards_from_redis(conn: &mut redis::Connection, user_id: i32) {
    let key = format!("user:{}:boards", user_id);
    conn.del::<&std::string::String, i32>(&key);
}

// Task handlers

pub fn put_board_tasks_to_redis(conn: &mut redis::Connection, board_id: i32, tasks: &[Task]) -> RedisResult<()> {
    let key = format!("board:{}:tasks", board_id);

    for task in tasks {
        let json = serde_json::to_string(task).unwrap();
        conn.rpush(&key, json)?;
    }
    conn.expire(&key, STORED_DATA_EXPIRATION_TIME)?;
    Ok(())
}

pub fn get_board_tasks_from_redis(conn: &mut redis::Connection, board_id: i32) -> RedisResult<Vec<Task>> {
    let key = format!("board:{}:tasks", board_id);
    let lifetime: i64 = conn.ttl(&key)?;
    if lifetime <= 0 {
        conn.del(&key)?;
    }

    let results: Vec<String> = conn.lrange(&key, 0, -1)?;

    let mut tasks_list = Vec::new();
    for res in results.iter() {
        let task: Task = serde_json::from_str(res).unwrap();
        tasks_list.push(task);
    }

    Ok(tasks_list)
}

pub fn drop_board_tasks_from_redis(conn: &mut redis::Connection, board_id: i32) {
    let key = format!("board:{}:tasks", board_id);
    conn.del::<&std::string::String, i32>(&key);
}