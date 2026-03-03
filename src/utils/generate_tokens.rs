use crate::utils::hashing_handler::hashing_handler;
use crate::utils::load_env::load_env;
use chrono::{Duration, Utc};
use jsonwebtoken::errors::Error as JwtError;
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: i64,
    pub email: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub email: String,
    // pub is_admin: Option<bool>,
    // pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Tokens {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub one_time_password_token: Option<String>,
    pub auth_cookie: Option<String>,
}

pub async fn generate_tokens(token_type: &str, user: User) -> Result<Tokens, JwtError> {
    load_env();

    let jwt_secret = env::var("APP__AUTH__JWT_SECRET").unwrap();
    let access_expiry = env::var("JWT_ACCESS_EXPIRATION_TIME").unwrap_or("1".to_string());
    let session_expiry = env::var("JWT_SESSION_EXPIRATION_TIME").unwrap_or("24".to_string());
    let otp_expiry = env::var("JWT_ONE_TIME_PASSWORD_LIFETIME").unwrap_or("5".to_string());

    let access_token_expiration = Utc::now()
        .checked_add_signed(Duration::hours(access_expiry.parse().unwrap()))
        .unwrap()
        .timestamp() as usize;

    let refresh_token_expiration = Utc::now()
        .checked_add_signed(Duration::hours(session_expiry.parse().unwrap()))
        .unwrap()
        .timestamp() as usize;

    let otp_token_expiration = Utc::now()
        .checked_add_signed(Duration::minutes(otp_expiry.parse().unwrap()))
        .unwrap()
        .timestamp() as usize;

    match token_type {
        "auth" => {
            let access_claims = Claims {
                id: user.id,
                email: user.email.clone(),
                exp: access_token_expiration,
                iat: Utc::now().timestamp_millis() as usize,
            };

            let access_token = encode(
                &Header::default(),
                &access_claims,
                &EncodingKey::from_secret(jwt_secret.as_bytes()),
            )?;

            let refresh_claims = Claims {
                id: user.id,
                email: user.email.clone(),
                exp: refresh_token_expiration,
                iat: Utc::now().timestamp_millis() as usize,
            };

            let refresh_token = encode(
                &Header::default(),
                &refresh_claims,
                &EncodingKey::from_secret(jwt_secret.as_bytes()),
            )?;

            let auth_cookie_part_a = match hashing_handler(user.email.as_str()).await {
                Ok(hash) => hash.to_string(),
                Err(e) => e.to_string(),
            };

            let auth_cookie_part_b = match hashing_handler(&jwt_secret).await {
                Ok(hash) => hash.to_string(),
                Err(e) => e.to_string(),
            };

            let auth_cookie = format!(
                "rusty_chat____{ }____{ }",
                auth_cookie_part_a, auth_cookie_part_b
            );

            Ok(Tokens {
                access_token: Some(access_token),
                refresh_token: Some(refresh_token),
                one_time_password_token: None,
                auth_cookie: Some(auth_cookie),
            })
        }

        "one_time_password" => {
            let otp_claims = Claims {
                id: user.id,
                email: user.email.clone(),
                exp: otp_token_expiration,
                iat: Utc::now().timestamp_millis() as usize,
            };

            let otp_token = encode(
                &Header::default(),
                &otp_claims,
                &EncodingKey::from_secret(jwt_secret.as_bytes()),
            )?;

            Ok(Tokens {
                access_token: None,
                refresh_token: None,
                one_time_password_token: Some(otp_token),
                auth_cookie: None,
            })
        }

        _ => Ok(Tokens {
            access_token: None,
            refresh_token: None,
            one_time_password_token: None,
            auth_cookie: None,
        }),
    }
}
