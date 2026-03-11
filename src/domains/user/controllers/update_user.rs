use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use crate::utils::cookie_deploy_handler::deploy_auth_cookie;
use crate::utils::generate_tokens::{User, generate_tokens};

use crate::AppState;
use axum::extract::State;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;
use tracing::error;

#[derive(Debug, Deserialize)]
pub struct UpdateUserPayload {
    pub full_name: Option<String>,
    pub email: Option<String>,
    // pub password: Option<String>,
    pub country: Option<String>,
    pub phone_number: Option<String>,
    pub status: Option<String>,
    pub last_seen: Option<String>,
}

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
    is_admin: bool,
    is_active: bool,
    country: String,
    phone_number: String,
    is_logged_out: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, sqlx::FromRow)]
struct UserLookup {
    id: i64,
    email: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct UpdateResponse {
    response_message: String,
    response: Option<UserProfile>,
    error: Option<String>,
}

pub async fn update_user(
    cookies: Cookies,
    // Extension(db_pool): Extension<PgPool>,
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Path(user_id): Path<i64>,
    Json(payload): Json<UpdateUserPayload>,
) -> impl IntoResponse {
    // only an admin or owner of the profile can update
    if session.user.id != user_id && !session.user.is_admin {
        error!("UNAUTHORIZED USER UPDATE ATTEMPT!");

        return (
            StatusCode::UNAUTHORIZED,
            Json(UpdateResponse {
                response_message: "You're not permitted to perform this action for this user"
                    .into(),
                response: None,
                error: Some("Unauthorized user update attempt".into()),
            }),
        );
    }

    // Build dynamic SQL with proper parameter indexing
    let mut set_clauses = Vec::new();
    let mut param_index = 2; // Start at 2 because $1 is user_id

    if payload.full_name.is_some() {
        set_clauses.push(format!("full_name = ${}", param_index));
        param_index += 1;
    }

    if payload.country.is_some() {
        set_clauses.push(format!("country = ${}", param_index));
        param_index += 1;
    }

    if payload.phone_number.is_some() {
        set_clauses.push(format!("phone_number = ${}", param_index));
        param_index += 1;
    }

    if payload.status.is_some() {
        set_clauses.push(format!("status = ${}", param_index));
        param_index += 1;
    }

    if payload.last_seen.is_some() {
        set_clauses.push(format!("last_seen = ${}", param_index));
        param_index += 1;
    }

    // if payload.profile_image_url.is_some() {
    //     set_clauses.push(format!("profile_image_url = ${}", param_index));
    //     param_index += 1;
    // }
    if payload.email.is_some() {
        set_clauses.push(format!("email = ${}", param_index));
        param_index += 1;

        // also prepare for updating the access and refresh tokens since we'll be updating the email
        set_clauses.push(format!("access_token = ${}", param_index));
        param_index += 1;

        set_clauses.push(format!("refresh_token = ${}", param_index));
        param_index += 1;
    }

    // Let password have its own dedicated end-point
    // if payload.password.is_some() {
    //     set_clauses.push(format!("password = ${}", param_index));
    //     param_index += 1;
    // }

    if set_clauses.is_empty() {
        error!("USER UPDATE FAILED: EMPTY REQUEST PAYLOAD PROVIDED!");

        return (
            StatusCode::BAD_REQUEST,
            Json(UpdateResponse {
                response_message: "No fields were provided to update".into(),
                response: None,
                error: Some("Empty payload".into()),
            }),
        );
    }

    // Build the query
    let query = format!(
        r#"
        UPDATE users
        SET {}, updated_at = NOW()
        WHERE id = $1
        RETURNING id, full_name, email, profile_image,
        access_token, refresh_token, status, last_seen, is_active, is_admin, country, phone_number, is_logged_out, created_at, updated_at
"#,
        set_clauses.join(", ")
    );

    // Build query with bindings
    let mut query_builder = sqlx::query_as::<_, UserProfile>(&query).bind(user_id);

    if let Some(name) = payload.full_name {
        query_builder = query_builder.bind(name);
    }

    if let Some(country) = payload.country {
        query_builder = query_builder.bind(country);
    }

    if let Some(phone_number) = payload.phone_number {
        query_builder = query_builder.bind(phone_number);
    }

    if let Some(status) = payload.status {
        query_builder = query_builder.bind(status);
    }

    if let Some(last_seen) = payload.last_seen {
        query_builder = query_builder.bind(last_seen);
    }

    // if let Some(img) = payload.profile_image_url {
    //     query_builder = query_builder.bind(img);
    // }

    if let Some(email) = &payload.email {
        // handle regeneration for new user email before binding it in
        // 1. verify that the user exist
        let user_result = sqlx::query_as::<_, UserLookup>(
            "SELECT id, email, created_at, updated_at FROM users WHERE id = $1",
        )
        // user_id from request param
        .bind(user_id)
        .fetch_optional(&state.db)
        .await;

        let user = match user_result {
            Ok(Some(user)) => user,
            Ok(None) => {
                error!("PROFILE UPDATE FAILED: USER NOT FOUND!");

                return (
                    StatusCode::UNAUTHORIZED,
                    Json(UpdateResponse {
                        response_message: "Login failed".to_string(),
                        response: None,
                        error: Some("User not found, profile update failed".to_string()),
                    }),
                );
            }
            Err(e) => {
                error!("USER UPDATE FAILED!");

                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(UpdateResponse {
                        response_message: "User update failed".to_string(),
                        response: None,
                        error: Some(format!("Database error: {}", e)),
                    }),
                );
            }
        };

        // 2. generate tokens
        let tokens = match generate_tokens(
            "auth",
            User {
                id: user.id,
                email: user.email,
            },
            state.config.as_ref(),
        )
        .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                error!("TOKEN GENERATION ERROR!");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(UpdateResponse {
                        response_message: "Failed to generate tokens".to_string(),
                        response: None,
                        error: Some(format!("Token generation error: {}", e)),
                    }),
                );
            }
        };

        let auth_cookie = match tokens.auth_cookie {
            Some(cookie) => cookie,
            None => {
                error!("AUTH COOKIE GENERATION FAILED!");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(UpdateResponse {
                        response_message: "Failed to generate auth cookie".to_string(),
                        response: None,
                        error: Some("Auth cookie generation error".to_string()),
                    }),
                );
            }
        };

        deploy_auth_cookie(cookies, auth_cookie, state.config.as_ref()).await;

        query_builder = query_builder.bind(email);

        // also generate query bindings for the new access and refresh tokens
        if let Some(access_token) = tokens.access_token {
            query_builder = query_builder.bind(access_token);
        }

        if let Some(refresh_token) = tokens.refresh_token {
            query_builder = query_builder.bind(refresh_token);
        }
    }

    // Let password have its own dedicated end-point
    // if let Some(password) = &payload.password {
    //     let hashed_password = match
    //     hashing_handler(&payload.password
    //         .clone()
    //         .expect("Failed to hash password!")
    //         .to_string()).await {
    //         Ok(hash) => hash,
    //         Err(e) => {
    //             error!("PASSWORD HASHING ERROR!");
    //
    //             return (
    //                 StatusCode::BAD_REQUEST,
    //                 Json(UpdateResponse {
    //                     response_message: "Failed to hash password".to_string(),
    //                     response: None,
    //                     error: Some(format!("Password hashing error: {}", e)),
    //                 }),
    //             );
    //         }
    //     };
    //
    //     query_builder = query_builder.bind(hashed_password);
    // }

    // println!("request payload: {:?}", payload);

    // println!("set_clauses: {:?}", set_clauses);
    // println!("set_clauses: {:?}", set_clauses.join(", "));

    // Execute query
    let result = query_builder.fetch_optional(&state.db).await;

    match result {
        Ok(Some(updated_user)) => (
            StatusCode::OK,
            Json(UpdateResponse {
                response_message: "User updated successfully".into(),
                response: Some(updated_user),
                error: None,
            }),
        ),
        Ok(None) => {
            error!("USER NOT FOUND FOR UPDATE!");
            (
                StatusCode::NOT_FOUND,
                Json(UpdateResponse {
                    response_message: "User not found".into(),
                    response: None,
                    error: Some(format!("No user with id {}", user_id)),
                }),
            )
        }
        Err(e) => {
            error!("FAILED TO UPDATE USER!");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateResponse {
                    response_message: "Failed to update user".into(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            )
        }
    }
}
