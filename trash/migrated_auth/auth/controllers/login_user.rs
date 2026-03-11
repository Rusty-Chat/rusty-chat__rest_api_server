use crate::utils::generate_tokens::{User, generate_tokens};
use axum::extract::State;
use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
// utils import
use crate::AppState;
use crate::utils::cookie_deploy_handler::deploy_auth_cookie;
use crate::utils::verification_handler::verification_handler; // your existing password verification function
use chrono::NaiveDateTime;
use tower_cookies::Cookies;
use tracing::error;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    // #[sqlx(rename = "id")]
    id: i64,
    full_name: String,
    email: String,
    profile_image: Option<String>,
    #[serde(skip_serializing)]
    password: String,
    is_admin: bool,
    is_active: bool,
    status: String,
    country: String,
    phone_number: String,
    is_logged_out: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct ResponseCore {
    user_profile: UserProfile,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    response_message: String,
    response: Option<ResponseCore>,
    error: Option<String>,
}

// Reuse UserProfile and ResponseCore from register controller

pub async fn login_user(
    cookies: Cookies,
    // Extension(db_pool): Extension<PgPool>,
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    // Fetch user by email
    let user_result = sqlx::query_as::<_, UserProfile>(
        "SELECT id, full_name, email, profile_image, password, is_active, is_admin, country, phone_number, is_logged_out, status, created_at, updated_at FROM users WHERE email = $1",
    )
    .bind(&payload.email)
    .fetch_optional(&state.db)
    .await;

    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!("LOGIN FAILED: PROVIDE EMAIL AND PASSWORD!");

            return (
                StatusCode::UNAUTHORIZED,
                Json(LoginResponse {
                    response_message: "Login failed".to_string(),
                    response: None,
                    error: Some("Login failed - provide a correct email and password".to_string()),
                }),
            );
        }
        Err(e) => {
            error!("USER LOGIN FAILED!");

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LoginResponse {
                    response_message: "Login failed".to_string(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            );
        }
    };

    match verification_handler(&payload.password, &user.password).await {
        Ok(true) => {
            let tokens = match generate_tokens(
                "auth",
                User {
                    id: user.id,
                    email: payload.email.clone(),
                },
            )
            .await
            {
                Ok(tokens) => tokens,
                Err(e) => {
                    error!("TOKEN GENERATION ERROR!");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(LoginResponse {
                            response_message: "Failed to generate tokens".to_string(),
                            response: None,
                            error: Some(format!("Token generation error: {}", e)),
                        }),
                    );
                }
            };

            deploy_auth_cookie(cookies, tokens.auth_cookie.unwrap()).await;

            let _ = sqlx::query_as::<_, UserProfile>(
                r#"
                    UPDATE users
                    SET
                        access_token = $1,
                        refresh_token = $2,
                        is_logged_out = $3,
                        updated_at = NOW()
                    WHERE email = $4
                "#,
            )
            .bind(&tokens.access_token)
            .bind(&tokens.refresh_token)
            .bind(false)
            .bind(&payload.email)
            .fetch_one(&state.db)
            .await;

            (
                StatusCode::OK,
                Json(LoginResponse {
                    response_message: "Login successful".to_string(),
                    response: Some(ResponseCore {
                        user_profile: UserProfile {
                            is_logged_out: false,
                            ..user
                        },
                        access_token: tokens.access_token,
                        refresh_token: tokens.refresh_token,
                    }),
                    error: None,
                }),
            )
        }
        Ok(false) => {
            error!("USER LOGIN FAILED!");

            (
                StatusCode::UNAUTHORIZED,
                Json(LoginResponse {
                    response_message: "Login failed".to_string(),
                    response: None,
                    error: Some("Invalid email or password".to_string()),
                }),
            )
        }
        Err(e) => {
            error!("USER LOGIN FAILED!");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LoginResponse {
                    response_message: "Login failed".to_string(),
                    response: None,
                    error: Some(format!("Password verification error: {}", e)),
                }),
            )
        }
    }
}
