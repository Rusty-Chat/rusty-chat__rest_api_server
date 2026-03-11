//! # Token Generation
//!
//! This module handles the creation of JSON Web Tokens (JWTs) for authentication,
//! including access tokens, refresh tokens, and one-time passwords (OTPs).
//! It also generates specialized authentication cookies.

use crate::utils::hashing_handler::hashing_handler;
use crate::utils::load_config::AppConfig;
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("Hashing error: {0}")]
    Hashing(String),
    #[error("Auth configuration is missing")]
    MissingAuth,
    #[error("Invalid token type: {0}")]
    InvalidTokenType(String),
    #[error("Expiration calculation failed: {0}")]
    ExpirationCalculation(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum TokenKind {
    Access,
    Refresh,
    OneTimePassword,
}

/// JWT Claims structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// User ID.
    pub id: i64,
    /// User email address.
    pub email: String,
    /// Expiration timestamp (seconds since epoch).
    pub exp: usize,
    /// Issued-at timestamp (milliseconds since epoch).
    pub iat: usize,
    /// Token discriminator.
    pub token_kind: TokenKind,
}

/// Simplified User structure for token generation.
#[derive(Clone, Debug)]
pub struct User {
    /// User ID.
    pub id: i64,
    /// User email address.
    pub email: String,
}

/// Container for generated tokens and cookies.
#[derive(Debug, Serialize)]
pub struct Tokens {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub one_time_password_token: Option<String>,
    pub auth_cookie: Option<String>,
}

/// Generates tokens based on the requested `token_type`.
///
/// # Arguments
/// - `token_type`: Either `"auth"` for access/refresh tokens or `"one_time_password"` for OTP.
/// - `user`: The user for whom tokens are being generated.
/// - `config`: Application configuration for JWT secrets and lifetimes.
pub async fn generate_tokens(
    token_type: &str,
    user: User,
    config: &AppConfig,
) -> Result<Tokens, JwtError> {
    let auth = config.auth.as_ref().ok_or(JwtError::MissingAuth)?;

    let jwt_secret = &auth.jwt_secret;
    let access_expiry = auth.jwt_access_expiration_time_in_hours;
    let session_expiry = auth.jwt_session_expiration_time_in_hours;
    let otp_expiry = auth.jwt_one_time_password_lifetime_in_minutes;

    let now = Utc::now();

    let access_token_expiration = calculate_expiration(now, access_expiry, true)?;
    let refresh_token_expiration = calculate_expiration(now, session_expiry, true)?;
    let otp_token_expiration = calculate_expiration(now, otp_expiry, false)?;

    match token_type {
        "auth" => {
            let access_claims = Claims {
                id: user.id,
                email: user.email.clone(),
                exp: access_token_expiration,
                iat: now.timestamp() as usize,
                token_kind: TokenKind::Access,
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
                iat: now.timestamp() as usize,
                token_kind: TokenKind::Refresh,
            };

            let refresh_token = encode(
                &Header::default(),
                &refresh_claims,
                &EncodingKey::from_secret(jwt_secret.as_bytes()),
            )?;

            let auth_cookie_part_a = hashing_handler(user.email.as_str())
                .await
                .map_err(|e| JwtError::Hashing(e.to_string()))?;

            let auth_cookie_part_b = hashing_handler(jwt_secret)
                .await
                .map_err(|e| JwtError::Hashing(e.to_string()))?;

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
                iat: now.timestamp() as usize,
                token_kind: TokenKind::OneTimePassword,
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

        token_type => Err(JwtError::InvalidTokenType(token_type.to_string())),
    }
}

/// Safely calculates expiration timestamp.
fn calculate_expiration(
    now: chrono::DateTime<Utc>,
    amount: u64,
    is_hours: bool,
) -> Result<usize, JwtError> {
    let amount_i64 = i64::try_from(amount)
        .map_err(|_| JwtError::ExpirationCalculation("Config value too large".into()))?;

    let duration = if is_hours {
        Duration::try_hours(amount_i64)
    } else {
        Duration::try_minutes(amount_i64)
    }
    .ok_or_else(|| JwtError::ExpirationCalculation("Duration overflow".into()))?;

    now.checked_add_signed(duration)
        .ok_or_else(|| JwtError::ExpirationCalculation("Timestamp overflow".into()))
        .map(|dt| dt.timestamp() as usize)
}
