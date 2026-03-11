use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Debug, Deserialize)]
pub struct ReactToMessagePayload {
    pub reaction_type: String, // e.g., "👍", "❤️", "😂", etc.
    pub sender_id: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Message {
    pub id: i64,
    pub room_id: i64,
    pub sender_id: Option<i64>,
    #[sqlx(rename = "type")]
    pub message_type: String,
    pub text_content: Option<String>,
    pub attachment_1: Option<String>,
    pub attachment_2: Option<String>,
    pub attachment_3: Option<String>,
    pub attachment_4: Option<String>,
    pub updates_counter: i32,
    pub status: String,
    pub sent_at: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
    pub co_member: Option<i64>, // for private rooms only
    pub co_members: Option<Vec<i64>>,
    pub is_public: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MessageReaction {
    pub id: i64,
    pub message_id: i64,
    pub room_id: i64,
    pub message_updates_counter: i32,
    pub sender_id: i64,
    pub reaction_type: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ResponseCore {
    // reaction: Option<MessageReaction>,
    message: Option<Message>,
}

#[derive(Debug, Serialize)]
pub struct ReactToMessageResponse {
    pub response_message: String,
    pub response: Option<ResponseCore>,
    pub error: Option<String>,
}

pub async fn react_to_message(
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Path(message_id): Path<i64>,
    Json(payload): Json<ReactToMessagePayload>,
) -> impl IntoResponse {
    // 1. Fetch current message
    let message_result = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1")
        .bind(message_id)
        .fetch_optional(&state.db)
        .await;

    let message = match message_result {
        Ok(Some(m)) => m,
        Ok(None) => {
            error!("MESSAGE NOT FOUND!");

            return (
                StatusCode::NOT_FOUND,
                Json(ReactToMessageResponse {
                    response_message: "Message not found".to_string(),
                    response: None,
                    error: Some("Message not found".to_string()),
                }),
            );
        }
        Err(e) => {
            error!("DATABASE ERROR: {}", e);

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ReactToMessageResponse {
                    response_message: "Database error".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // 2. Get room
    let room_res = sqlx::query_as::<_, Room>(
        r#"
        SELECT * FROM rooms WHERE id = $1
        "#,
    )
    .bind(&message.room_id)
    .fetch_one(&state.db)
    .await;

    let room = match room_res {
        Ok(r) => r,
        Err(e) => {
            error!("FAILED TO GET ROOM: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ReactToMessageResponse {
                    response_message: "Failed to get room".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // 3. Check if user is authorized to react (must be in the room)
    let is_authorized = if room.is_group {
        room.co_members
            .as_ref()
            .map(|members| members.contains(&payload.sender_id))
            .unwrap_or(false)
            || message.sender_id == Some(payload.sender_id)
    } else {
        room.co_member == Some(payload.sender_id) || message.sender_id == Some(payload.sender_id)
    };

    if !is_authorized {
        error!("UNAUTHORIZED REACTION ATTEMPT!");

        return (
            StatusCode::FORBIDDEN,
            Json(ReactToMessageResponse {
                response_message: "You are not authorized to react to this message".to_string(),
                response: None,
                error: Some("FORBIDDEN".to_string()),
            }),
        );
    }

    // 4. Check if user has already reacted to this message
    let existing_reaction = sqlx::query_as::<_, MessageReaction>(
        "SELECT * FROM message_reactions WHERE message_id = $1 AND sender_id = $2",
    )
    .bind(message_id)
    .bind(&payload.sender_id)
    .fetch_optional(&state.db)
    .await;

    let new_updates_count = message.updates_counter + 1;

    let reaction_result = match existing_reaction {
        Ok(Some(_)) => {
            // Update existing reaction
            sqlx::query_as::<_, MessageReaction>(
                "UPDATE message_reactions SET reaction_type = $1, updated_at = NOW() 
                 WHERE message_id = $2 AND sender_id = $3 AND message_updates_counter = $4 
                 RETURNING *",
            )
            .bind(&payload.reaction_type)
            .bind(message_id)
            .bind(&payload.sender_id)
            .bind(new_updates_count)
            .fetch_one(&state.db)
            .await
        }
        Ok(None) => {
            // Create new reaction
            sqlx::query_as::<_, MessageReaction>(
                "INSERT INTO message_reactions (message_id, room_id, sender_id, reaction_type, message_updates_counter) 
                 VALUES ($1, $2, $3, $4, $5) 
                 RETURNING *"
            )
            .bind(message_id)
            .bind(&room.id)
            .bind(&payload.sender_id)
            .bind(&payload.reaction_type)
            .bind(new_updates_count)
            .fetch_one(&state.db)
            .await
        }
        Err(e) => {
            error!("FAILED TO CHECK EXISTING REACTION: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ReactToMessageResponse {
                    response_message: "Failed to check existing reaction".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    match reaction_result {
        Ok(r) => r,
        Err(e) => {
            error!("FAILED TO SAVE REACTION: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ReactToMessageResponse {
                    response_message: "Failed to save reaction".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // 5. Update the message status
    let update_message_status_res = sqlx::query_as::<_, Message>(
        r#"
        UPDATE messages
        SET status = 'reacted'
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(message_id)
    .fetch_one(&state.db)
    .await;

    let updated_message = match update_message_status_res {
        Ok(message) => message,
        Err(e) => {
            error!("FAILED TO UPDATE MESSAGE STATUS!");
            return (
                StatusCode::OK,
                Json(ReactToMessageResponse {
                    response_message:
                        "Reaction added successfully but failed to update message status"
                            .to_string(),
                    response: Some(ResponseCore {
                        // reaction: Some(reaction),
                        message: None,
                    }),
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // 6. Create reaction status receipts
    let receipt_res: Result<(), sqlx::Error>;
    // let new_updates_count = message.updates_counter + 1;

    match room.is_group {
        false => {
            // For private rooms, create receipt for the other user
            let receiver_id = if room.co_member == Some(payload.sender_id) {
                message.sender_id
            } else {
                room.co_member
            };

            receipt_res = sqlx::query(
                r#"
                INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status, updates_count_tracker)
                VALUES ($1, $2, $3, $4, 'reaction', 'reacted', $5)
                "#
            )
            .bind(message_id)
            .bind(&payload.sender_id)
            .bind(receiver_id)
            .bind(&message.room_id)
            .bind(new_updates_count)
            .execute(&state.db)
            .await
            .map(|_| ());
        }
        true => {
            // For group rooms, create receipts for all members except the sender
            let mut temp_res = Ok(());
            if let Some(members) = &room.co_members {
                for room_member in members {
                    // Skip the sender
                    // if *room_member == payload.sender_id {
                    //     continue;
                    // }

                    let res = sqlx::query(
                        r#"
                        INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status, updates_count_tracker)
                        VALUES ($1, $2, $3, $4, 'reaction', 'reacted', $5)
                        "#
                    )
                    .bind(message_id)
                    .bind(&payload.sender_id)
                    .bind(room_member)
                    .bind(&message.room_id)
                    .bind(new_updates_count)
                    .execute(&state.db)
                    .await;

                    if let Err(e) = res {
                        temp_res = Err(e);
                        break;
                    }
                }
            }
            receipt_res = temp_res;
        }
    }

    match receipt_res {
        Ok(_) => {
            (
                StatusCode::OK,
                Json(ReactToMessageResponse {
                    response_message: "Reaction added successfully".to_string(),
                    response: Some(ResponseCore {
                        // reaction: Some(reaction),
                        message: Some(updated_message),
                    }),
                    error: None,
                }),
            )
        }
        Err(e) => {
            error!(
                "REACTION CREATED SUCCESSFULLY, BUT FAILED TO CREATE MESSAGE STATUS RECEIPT: {}",
                e
            );

            (
                StatusCode::OK,
                Json(ReactToMessageResponse {
                    response_message:
                        "Reaction added successfully but failed to create message status receipt"
                            .to_string(),
                    response: Some(ResponseCore {
                        // reaction: None,
                        message: Some(updated_message),
                    }),
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}
