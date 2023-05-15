use actix_web::{
    dev::ServiceRequest,
    error::Error
};
use actix_web_httpauth::{
    extractors::{
        bearer::{self, BearerAuth},
        AuthenticationError,
    }
};

use http::header::{HeaderName, HeaderValue};
use jwt::{self, SignWithKey, VerifyWithKey};
use serde::{Serialize, Deserialize};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::str::FromStr;
use chrono;

use crate::{TOKEN_LIFETIME, TOKEN_UPDATE_LIFETIME_THRESHOLD};

#[derive(Serialize, Deserialize, Clone)]
pub struct JWToken {
    user_id: i32, 
    iat: i64
}

impl JWToken {
    pub fn new(user_id: i32, iat: i64) -> Self {
        JWToken { user_id, iat }
    }
}

pub fn create_jwt(token_body: JWToken) -> String {
    let jwt_secret: Hmac<Sha256> = Hmac::new_from_slice(
        std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET must be set!")
            .as_bytes(),
    ).unwrap();
    let token = token_body.sign_with_key(&jwt_secret).unwrap();

    token
}

pub fn check_jwt(jwtoken: String) -> Result<JWToken, String> {
    let jwt_secret: Hmac<Sha256> = Hmac::new_from_slice(
        std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET must be set!")
            .as_bytes(),
    ).unwrap();
    let user_data = jwtoken
        .verify_with_key(&jwt_secret)
        .map_err(|_| "Invalid token".to_string());

    user_data
}

pub async fn validate_user(
    mut request: ServiceRequest, 
    credentials: BearerAuth
) ->  Result<ServiceRequest, (Error, ServiceRequest)> {

    let jwtoken = credentials.token().to_string();

    match check_jwt(jwtoken) {
        Ok(token) => {
            let current_time = chrono::offset::Utc::now().naive_utc().timestamp();
            let time_delta = current_time - token.iat;
            if time_delta > TOKEN_LIFETIME {
                let config = request
                    .app_data::<bearer::Config>()
                    .cloned()
                    .unwrap_or_default()
                    .scope("");

                Err((AuthenticationError::from(config).into(), request))
            } else {
                let headers = request.headers_mut();
                let user_key = HeaderName::from_str("user_id").unwrap();
                let user_value = HeaderValue::from_str(&(token.user_id.to_string())).unwrap();
                headers.append(user_key, user_value);

                if time_delta > TOKEN_UPDATE_LIFETIME_THRESHOLD {
                    let updated_token = create_jwt(JWToken::new(
                        token.user_id, 
                        chrono::offset::Utc::now().naive_utc().timestamp()
                    ));
                    
                    let token_key = HeaderName::from_str("new_token").unwrap();
                    let token_value = HeaderValue::from_str(&updated_token).unwrap();
                    headers.append(token_key, token_value);
                    println!(" ------> TOKEN UPDATED!!! ");
                } 

                Ok(request)
            }
        },
        Err(_) => {
            let config = request
                .app_data::<bearer::Config>()
                .cloned()
                .unwrap_or_default()
                .scope("");

            Err((AuthenticationError::from(config).into(), request))
        }
    }
}
