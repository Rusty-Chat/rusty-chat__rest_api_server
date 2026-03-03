use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
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

#[derive(Debug, Deserialize)]
pub struct UpdateRoomPayload {
    pub room_name: Option<String>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Room {
    pub id: i64,
    pub room_name: Option<String>,
    pub is_group: bool,
    pub created_by: Option<i64>,
    pub bookmarked_by: Vec<i64>,
    pub archived_by: Vec<i64>,
    pub pinned_by: Vec<i64>,
    pub room_profile_image: Option<String>,
    pub co_member: Option<i64>,
    pub co_members: Option<Vec<i64>>,
    pub is_public: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, sqlx::FromRow)]
struct RoomLookup {
    id: i64,
    created_by: Option<i64>,
}

#[derive(Debug, sqlx::FromRow)]
struct RoomMemberLookup {
    role: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateResponse {
    response_message: String,
    response: Option<Room>,
    error: Option<String>,
}

pub async fn update_room(
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Path(room_id): Path<i64>,
    Json(payload): Json<UpdateRoomPayload>,
) -> impl IntoResponse {
    // 1. Verify room exists
    let room_result =
        sqlx::query_as::<_, RoomLookup>("SELECT id, created_by FROM rooms WHERE id = $1")
            .bind(room_id)
            .fetch_optional(&state.db)
            .await;

    let room = match room_result {
        Ok(Some(room)) => room,
        Ok(None) => {
            error!("ROOM UPDATE FAILED: ROOM NOT FOUND!");
            return (
                StatusCode::NOT_FOUND,
                Json(UpdateResponse {
                    response_message: "Room not found".to_string(),
                    response: None,
                    error: Some("Room update failed".to_string()),
                }),
            );
        }
        Err(e) => {
            error!("ROOM UPDATE FAILED: DATABASE ERROR!");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateResponse {
                    response_message: "Room update failed".to_string(),
                    response: None,
                    error: Some(format!("Server error: {}", e)),
                }),
            );
        }
    };

    // 2. Check if user is the room creator or app admin
    let member_result = sqlx::query_as::<_, RoomMemberLookup>(
        "SELECT role FROM room_members WHERE room_id = $1 AND user_id = $2",
    )
    .bind(room.id)
    .bind(session.user.id)
    .fetch_optional(&state.db)
    .await;

    let is_authorized = match member_result {
        Ok(Some(member)) => {
            member.role == "admin"
                || Some(session.user.id) == room.created_by
                || session.user.is_admin
        }
        Ok(None) => session.user.is_admin, // App admin can update even if not a member
        Err(_) => false,
    };

    if !is_authorized {
        error!("UNAUTHORIZED ROOM UPDATE ATTEMPT!");
        return (
            StatusCode::UNAUTHORIZED,
            Json(UpdateResponse {
                response_message: "Only room admin or creator can update room details".into(),
                response: None,
                error: Some("Unauthorized room update attempt".into()),
            }),
        );
    }

    // 3. Build dynamic SQL
    let mut set_clauses = Vec::new();
    let mut param_index = 2; // $1 is room_id

    if payload.room_name.is_some() {
        set_clauses.push(format!("room_name = ${}", param_index));
        param_index += 1;
    }

    if payload.is_public.is_some() {
        set_clauses.push(format!("is_public = ${}", param_index));
        // param_index += 1;
    }

    if set_clauses.is_empty() {
        error!("ROOM UPDATE FAILED: EMPTY PAYLOAD!");

        return (
            StatusCode::BAD_REQUEST,
            Json(UpdateResponse {
                response_message: "No fields provided to update".into(),
                response: None,
                error: Some("Empty payload".into()),
            }),
        );
    }

    let query = format!(
        r#"
        UPDATE rooms
        SET {}, updated_at = NOW()
        WHERE id = $1
        RETURNING id, room_name, is_group, created_by, bookmarked_by, archived_by, pinned_by, room_profile_image, co_member, co_members, is_public, created_at, updated_at
        "#,
        set_clauses.join(", ")
    );

    let mut query_builder = sqlx::query_as::<_, Room>(&query).bind(room.id);

    if let Some(room_name) = payload.room_name {
        query_builder = query_builder.bind(room_name);
    }

    if let Some(is_public) = payload.is_public {
        query_builder = query_builder.bind(is_public);
    }

    // 4. Execute update
    let result = query_builder.fetch_one(&state.db).await;

    match result {
        Ok(updated_room) => (
            StatusCode::OK,
            Json(UpdateResponse {
                response_message: "Room updated successfully".into(),
                response: Some(updated_room),
                error: None,
            }),
        ),
        Err(e) => {
            error!("FAILED TO UPDATE ROOM: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateResponse {
                    response_message: "Failed to update room".into(),
                    response: None,
                    error: Some(format!("Database error: {}", e)),
                }),
            )
        }
    }
}
