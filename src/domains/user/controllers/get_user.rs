use crate::AppState;
use axum::extract::State;
use axum::{Json, extract::Path, http::StatusCode, response::IntoResponse};
use chrono::NaiveDateTime;
use serde::Serialize;
use tracing::error;
// use crate::middlewares::auth_sessions_middleware::SessionUser;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    id: i64,
    full_name: String,
    email: String,
    profile_image: Option<String>,
    access_token: String,
    refresh_token: String,
    status: String,
    last_seen: Option<String>,
    #[serde(skip_serializing)]
    password: String,
    is_admin: bool,
    is_active: bool,
    country: String,
    phone_number: String,
    is_logged_out: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
#[derive(Debug, Serialize)]
pub struct UserResponse {
    response_message: String,
    response: Option<UserProfile>,
    error: Option<String>,
}


pub async fn get_user(
    // Extension(session): Extension<SessionUser>,
    // Extension(db_pool): Extension<PgPool>,
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    // req: Request,
) -> impl IntoResponse {
    let user_result = sqlx::query_as::<_, UserProfile>(
        "SELECT id, full_name, email, profile_image, password, access_token, refresh_token, status, last_seen, is_active, is_admin, country, phone_number, is_logged_out, created_at, updated_at FROM users WHERE id = $1"
    )
        .bind(user_id)
        .fetch_optional(&state.db)
        .await;

    match user_result {
        Ok(Some(user)) => (
            StatusCode::OK,
            Json(UserResponse {
                response_message: "User fetched successfully".to_string(),
                response: Some(user),
                error: None,
            }),
        ),
        Ok(None) => {
            error!("USER NOT FOUND!");

            (
                StatusCode::NOT_FOUND,
                Json(UserResponse {
                    response_message: "User not found".to_string(),
                    response: None,
                    error: Some(format!("No user with id: {}", user_id)),
                }),
            )
        }
        Err(e) => {
            error!("FAILED TO FETCH USER PROFILE!");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UserResponse {
                    response_message: "Failed to fetch user".to_string(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            )
        }
    }
}
