use serde::{Serialize, Deserialize};
use chrono::{NaiveDateTime, NaiveDate};
use sqlx::types::uuid::timestamp;

use crate::authorisation::UserWebData;

// Common

#[derive(Serialize)]
pub struct ServerResponse {
    pub status: i32, 
    pub message: String
}

// Users

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: i32, 
    pub name: String,
    pub email: String, 
    pub passwd: String,
    pub verification_status_id: i32,
    pub status_id: i32, 
    pub creation_time: i64
}

#[derive(Serialize, Deserialize)]
pub struct UserCredentials {
    pub email: String, 
    pub password: String
}

#[derive(Serialize, Deserialize)]
pub struct StoredUser {
    pub id: i32, 
    pub name: Option<String>, 
    pub email: Option<String>, 
    pub passwd: Option<String>,
    pub verification_status_id: Option<i32>,
    pub status_id: Option<i32>, 
    pub creation_time: Option<NaiveDateTime>
}

impl StoredUser {
    pub fn get_user(&self) -> User {
        User { 
            id: self.id, 
            name: self.name.clone().unwrap(), 
            email: self.email.clone().unwrap(), 
            passwd: self.passwd.clone().unwrap(), 
            verification_status_id: self.verification_status_id.unwrap(), 
            status_id: self.status_id.unwrap(), 
            creation_time: self.creation_time.unwrap_or_else(|| {
                NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()
            }).timestamp()
        }
    }
}

#[derive(Deserialize)]
pub struct CreateUserBody {
    pub name: String, 
    pub email: String, 
    pub password: String
}

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub user_data: UserWebData,
    pub old_password: String,
    pub new_password: String
}

#[derive(Deserialize)]
pub struct ChangeForgottenPasswordBody {
    pub email: String
}

#[derive(Deserialize)]
pub struct ChangeEmailBody {
    pub user_data: UserWebData,
    pub new_email: String
}

#[derive(Deserialize)]
pub struct ChangeUsernameBody {
    pub user_data: UserWebData,
    pub new_name: String
}

#[derive(Deserialize)]
pub struct DeleteUserBody {
    pub user_data: UserWebData
}

// Boards

#[derive(Serialize, Deserialize)]
pub struct Board {
    pub id: i32, 
    pub title: String, 
    pub description: String,
    pub creation_time: i64
}

#[derive(Serialize, Deserialize)]
pub struct StoredBoard {
    pub id: i32, 
    pub title: Option<String>, 
    pub description: Option<String>,
    pub creation_time: Option<NaiveDateTime>
}

impl StoredBoard {
    pub fn get_board(&self) -> Board {
        Board {
            id: self.id, 
            title: self.title.clone().unwrap_or_else(|| {"Unnamed board".to_string()}), 
            description: self.description.clone().unwrap_or_else(|| {"".to_string()}), 
            creation_time: self.creation_time.unwrap_or_else(|| {
                NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()
            }).timestamp()
        }
    }
}

#[derive(Deserialize)]
pub struct CreateBoardBody {
    pub user_data: UserWebData,
    pub title: String, 
    pub description: String
}

#[derive(Deserialize)]
pub struct UpdateBoardBody {
    pub user_data: UserWebData,
    pub id: i32, 
    pub title: String, 
    pub description: String
}

#[derive(Deserialize)]
pub struct DeleteBoardBody {
    pub user_data: UserWebData,
    pub id: i32
}


// Tasks

#[derive(Serialize, Deserialize)]
pub struct Task {
    pub id: i32, 
    pub title: String, 
    pub description: String,
    pub board_id: i32, 
    pub status_id: i32, 
    pub last_status_change_time: i64
}

#[derive(Serialize, Deserialize)]
pub struct StoredTask {
    pub id: i32, 
    pub title: Option<String>, 
    pub description: Option<String>,
    pub board_id: Option<i32>, 
    pub status_id: Option<i32>, 
    pub last_status_change_time: Option<NaiveDateTime>
}

impl StoredTask {
    pub fn get_task(&self) -> Task {
        Task {
            id: self.id, 
            title: self.title.clone().unwrap_or_else(|| {"Unnamed task".to_string()}), 
            description: self.description.clone().unwrap_or_else(|| {"".to_string()}), 
            board_id: self.board_id.unwrap_or_else(|| {0}),
            status_id: self.status_id.unwrap_or_else(|| {0}),
            last_status_change_time: self.last_status_change_time.unwrap_or_else(|| {
                NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap()
            }).timestamp()
        }
    }
}

#[derive(Deserialize)]
pub struct GetTasksBody {
    pub user_data: UserWebData,
    pub board_id: i32
}

#[derive(Deserialize)]
pub struct CreateTaskBody {
    pub user_data: UserWebData,
    pub board_id: i32, 
    pub title: String, 
    pub description: String
}

#[derive(Deserialize)]
pub struct UpdateTaskBody {
    pub user_data: UserWebData,
    pub id: i32, 
    pub board_id: i32, 
    pub title: String, 
    pub description: String, 
    pub status_id: i32
}

#[derive(Deserialize)]
pub struct DeleteTaskBody {
    pub user_data: UserWebData,
    pub id: i32, 
    pub board_id: i32
}
