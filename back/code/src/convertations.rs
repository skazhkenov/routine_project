use sha2::{Digest, Sha256};
use base64::{Engine as _, alphabet, engine::{self, general_purpose}};

pub trait AsHash {
    fn as_hash(&self) -> Self;
}

impl AsHash for String {
    fn as_hash(&self) -> Self {
    
        let bytes_value = self.as_bytes();
        let mut hasher = Sha256::new();
        hasher.update(bytes_value);

        
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}

pub trait AsBase64 {
    fn as_base64(&self) -> Self;
}

impl AsBase64 for String {
    fn as_base64(&self) -> Self {
        let b64_string = general_purpose::STANDARD.encode(self);
        
        b64_string
    }
}

pub trait FromBase64 
where Self: std::marker::Sized {
    fn from_base64(&self) -> Option<Self>;
}

impl FromBase64 for String {
    fn from_base64(&self) -> Option<Self> {
        let decode_result = general_purpose::STANDARD
            .decode(self);
        match decode_result {
            Ok(bytes) => {
                let string: String = bytes
                    .into_iter()
                    .map(|byte| {
                        let character: char = byte.into();
                        character
                    })
                    .collect();
                Some(string)
            }, 
            Err(_) => {
                None
            }
        }
    }
}