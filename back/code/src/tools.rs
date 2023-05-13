use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

pub fn send_email(email: String, title: &str, message: &str) {
    
    println!("Verification mail for user sent to {}. {}", email, message);

    let login = std::env::var("LOGIN").expect("Unable to read LOGIN env var");
    let password = std::env::var("PASSWORD").expect("Unable to read PASSWORD env var");

    let email = Message::builder()
        .from((&format!("Admin <{}>", login)).parse().unwrap())
        .to((&format!("User <{}>", email)).parse().unwrap())
        .subject(title)
        .header(ContentType::TEXT_PLAIN)
        .body(String::from(message))
        .unwrap();

    let creds = Credentials::new(login, password);

    // Open a remote connection to gmail
    let mailer = SmtpTransport::relay("smtp.mail.ru")
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => println!("Could not send email: {e:?}"),
    }

}

pub fn generate_random_string() -> String {
    String::from("0123456789")
}