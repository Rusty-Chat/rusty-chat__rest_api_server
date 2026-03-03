use crate::domains::rooms::controllers::update_room::UpdateResponse as RoomUpdateResponse;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use crate::utils::hashing_handler::hashing_handler;

use crate::AppState;
use crate::utils::verification_handler::verification_handler;
use axum::extract::State;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::error;


#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LocalUserProfile {
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
pub struct PasswordUpdateResponse {
    response_message: String,
    response: Option<LocalUserProfile>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordPayload {
    pub old_password: String,
    pub new_password: String,
}

/// update_password is a function that updates a user's password.
pub async fn update_password(
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Path(user_id): Path<i64>,
    Json(payload): Json<UpdatePasswordPayload>,
) -> impl IntoResponse {
    let user_result = sqlx::query_as::<_, LocalUserProfile>(
        r#"
        SELECT id, full_name, email, profile_image, password,
               access_token, refresh_token, status, last_seen,
               is_active, is_admin, country, phone_number, is_logged_out, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await;

    let (user, password_matches) = match user_result {
        Ok(Some(user)) => {
            // Check authorization first
            if session.user.email != user.email && !session.user.is_admin {
                error!("UNAUTHORIZED PASSWORD UPDATE ATTEMPT!");
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(PasswordUpdateResponse {
                        response_message: "You're not permitted to perform this action".into(),
                        response: None,
                        error: Some("Unauthorized password update attempt".into()),
                    }),
                );
            }

            let password_matches = match verification_handler(&payload.old_password, &user.password).await {
                Ok(valid) => valid,
                Err(e) => {
                    error!("PASSWORD VERIFICATION ERROR ON PASSWORD RESET!");

                    return (
                        StatusCode::BAD_REQUEST,
                        Json(PasswordUpdateResponse {
                            response_message: "Password verification failed".into(),
                            response: None,
                            error: Some(format!("Password verification error: {}", e)),
                        }),
                    );
                }
            };

            (user, password_matches)
        },
        Ok(None) => {
            error!("FAILED TO FETCH USER FOR PASSWORD UPDATE!");

            return (
                StatusCode::NOT_FOUND,
                Json(PasswordUpdateResponse {
                    response_message: "User not found".into(),
                    response: None,
                    error: Some("No user with this id".into()),
                }),
            );
        }
        Err(e) => {
            error!("FAILED TO FETCH USER FOR PASSWORD UPDATE!");

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PasswordUpdateResponse {
                    response_message: "Failed to fetch user".into(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            );
        }
    };

    if !password_matches {
        error!("PASSWORD RESET ATTEMPT WITH INVALID OLD PASSWORD!");

        return (
            StatusCode::UNAUTHORIZED,
            Json(PasswordUpdateResponse {
                response_message: "Invalid old password".into(),
                response: None,
                error: Some("Old password does not match".into()),
            }),
        );
    }

    let hashed_password = match hashing_handler(&payload.new_password).await {
        Ok(hash) => hash,
        Err(e) => {
            error!("NEW-PASSWORD HASHING ERROR ON PASSWORD RESET!");

            return (
                StatusCode::BAD_REQUEST,
                Json(PasswordUpdateResponse {
                    response_message: "Failed to hash password".into(),
                    response: None,
                    error: Some(format!("Password hashing error: {}", e)),
                }),
            );
        }
    };

    let updated_user = match sqlx::query_as::<_, LocalUserProfile>(
        r#"
        UPDATE users
        SET password = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, full_name, email, profile_image, password,
                  access_token, refresh_token, status, last_seen,
                  is_active, is_admin, country, phone_number, is_logged_out, created_at, updated_at
        "#,
    )
    .bind(hashed_password)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    {
        Ok(user) => user,
        Err(e) => {
            error!("FAILED TO UPDATE PASSWORD!");

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PasswordUpdateResponse {
                    response_message: "Failed to update password".into(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            );
        }
    };

    (
        StatusCode::OK,
        Json(PasswordUpdateResponse {
            response_message: "Password updated successfully".into(),
            response: Some(updated_user),
            error: None,
        }),
    )
}
