use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use uuid::Uuid;
use regex::Regex;
use log;

pub fn generate_random_password() -> String {
    let new_password = Uuid::new_v4().to_string();
    new_password
}

pub fn is_valid_password(password: &str) -> bool {
    if password.len() < 10 || password.len() > 64 {
        return false;
    }
    let re = Regex::new(r"^([a-zA-Z0-9._+\-!?]+)$").unwrap();
    re.is_match(password)
}

pub fn is_valid_email(email: &str) -> bool {
    let re = Regex::new(r"^([a-zA-Z0-9._%+-]+)@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})$").unwrap();
    re.is_match(email)
}

pub fn send_email(email: &str, title: &str, message: &str) {
    
    let login = std::env::var("LOGIN").expect("Unable to read LOGIN env var");
    let password = std::env::var("PASSWORD").expect("Unable to read PASSWORD env var");

    let common_message = Message::builder()
        .from((&format!("Admin <{}>", login)).parse().unwrap())
        .to((&format!("User <{}>", email)).parse().unwrap())
        .subject(title)
        .header(ContentType::TEXT_PLAIN)
        .body(String::from(message))
        .unwrap();

    let creds = Credentials::new(login, password);
    let mailer = SmtpTransport::relay("smtp.mail.ru")
        .unwrap()
        .credentials(creds)
        .build();

    match mailer.send(&common_message) {
        Ok(_) => {
            log::info!("Email to address {} successfully sent", email);
        },
        Err(e) => {
            log::error!("Could not send email to address {}: {e:?}", email);
        }
    }
}
