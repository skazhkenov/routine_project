use redis::{Commands, RedisResult};
use crate::models::{Board, Task};
use serde_json;

const STORED_DATA_EXPIRAION_TIME: usize = 600; // 10 min cache lifetime

// Board handlers

pub fn put_user_boards_to_redis(conn: &mut redis::Connection, user_id: i32, boards: &[Board]) -> RedisResult<()> {
    let key = format!("user:{}:boards", user_id);

    for board in boards {
        let json = serde_json::to_string(board).unwrap();
        conn.rpush(&key, json)?;
    }
    conn.expire(&key, STORED_DATA_EXPIRAION_TIME)?;
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
    conn.expire(&key, STORED_DATA_EXPIRAION_TIME)?;
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