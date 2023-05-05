use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct UserWebData {
    pub user_id: i32, 
    pub token: String
}

impl UserWebData {
    pub fn id(&self) -> i32 {
        self.user_id
    } 
    pub fn token(&self) -> String {
        self.token.clone()
    }
}

pub fn check_token(web_token_data: UserWebData, stored_token_data: UserWebData) -> bool {
    let user_id = web_token_data.user_id;
    true
}