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
pub struct UpdateMessagePayload {
    pub text_content: String,
    pub sender_id: i64,
}

#[derive(sqlx::Type, Debug)]
#[sqlx(type_name = "text")]
pub enum MessageAction {
    #[sqlx(rename = "original-send")]
    OriginalSend,
    #[sqlx(rename = "edit")]
    Edit,
    #[sqlx(rename = "delete")]
    Delete,
    #[sqlx(rename = "reaction")]
    Reaction,
    #[sqlx(rename = "system")]
    System,
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

#[derive(Debug, Serialize)]
pub struct UpdateMessageResponse {
    pub response_message: String,
    pub response: Option<Message>,
    pub error: Option<String>,
}

pub async fn update_message(
    State(state): State<AppState>,
    Extension(session): Extension<SessionsMiddlewareOutput>,
    Path(message_id): Path<i64>,
    Json(payload): Json<UpdateMessagePayload>,
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
                Json(UpdateMessageResponse {
                    response_message: "Message not found".to_string(),
                    response: None,
                    error: Some("Message not found".to_string()),
                }),
            );
        }
        Err(e) => {
            error!("DATABASE ERROR!");

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateMessageResponse {
                    response_message: "Database error".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // 2. Check permissions (only sender can update)
    if message.sender_id != Some(payload.sender_id) {
        error!("UNAUTHORIZED MESSAGE UPDATE ATTEMPT!");

        return (
            StatusCode::FORBIDDEN,
            Json(UpdateMessageResponse {
                response_message: "You can only update your own messages".to_string(),
                response: None,
                error: Some("Forbidden".to_string()),
            }),
        );
    }

    // // 3. Update message and save history in a transaction
    // let mut tx = match state.db.begin().await {
    //     Ok(tx) => tx,
    //     Err(e) => {
    //         return (
    //             StatusCode::INTERNAL_SERVER_ERROR,
    //             Json(UpdateMessageResponse {
    //                 response_message: "Failed to start transaction".to_string(),
    //                 response: None,
    //                 error: Some(e.to_string()),
    //             }),
    //         );
    //     }
    // };

    // Save history
    let old_content = &message.text_content;
    let new_content = &payload.text_content;

    let history_res = sqlx::query(
        "INSERT INTO message_edits (message_id, previous_context, new_content) VALUES ($1, $2, $3)",
    )
    .bind(message_id)
    .bind(old_content)
    .bind(new_content)
    .execute(&state.db)
    .await;

    if let Err(e) = history_res {
        error!("FAILED TO SAVE MESSAGE TO EDIT HISTORY!");

        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(UpdateMessageResponse {
                response_message: "Failed to save edit history".to_string(),
                response: None,
                error: Some(e.to_string()),
            }),
        );
    }

    let new_updates_count = message.updates_counter + 1;
    let receipt_res: Result<(), sqlx::Error>;

    // get room
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
                Json(UpdateMessageResponse {
                    response_message: "Failed to get room".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // Update message
    let update_res = sqlx::query_as::<_, Message>(
        "UPDATE messages SET text_content = $1, updates_counter = $2, status = 'updated', updated_at = NOW() WHERE id = $3 RETURNING *",
    )
    .bind(&payload.text_content)
    .bind(&new_updates_count)
    .bind(message_id)
    .fetch_one(&state.db)
    .await;

    match update_res {
        Ok(updated_message) => {
            // Create "updated" status receipt(s)
            match room.is_group {
                false => {
                    receipt_res = sqlx::query(
                        r#"
                        INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status, updates_count_tracker)
                        VALUES ($1, $2, $3, $4, 'edit', 'updated', $5)
                        "#
                    )
                    .bind(updated_message.id)
                    .bind(&payload.sender_id)
                    .bind(room.co_member)
                    .bind(&message.room_id)
                    .bind(&new_updates_count)
                    .execute(&state.db)
                    .await
                    .map(|_| ());
                }
                true => {
                    let mut temp_res = Ok(());
                    if let Some(members) = &room.co_members {
                        for room_member in members {
                            let res = sqlx::query(
                                r#"
                                INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status, updates_count_tracker)
                                VALUES ($1, $2, $3, $4, 'edit', 'updated', $5)
                                "#
                            )
                            .bind(updated_message.id)
                            .bind(&payload.sender_id)
                            .bind(room_member)
                            .bind(message.room_id)
                            .bind(&new_updates_count)
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
                Ok(_) => {}
                Err(_e) => {
                    error!(
                        "MESSAGE UPDATED SUCCESSFULLY, BUT FAILED TO CREATE MESSAGE STATUS RECEIPT!"
                    );

                    return (
                           StatusCode::OK,
                           Json(UpdateMessageResponse {
                               response_message: "Message updated successfully but failed to create message status receipt".to_string(),
                               response: Some(updated_message),
                               error: None,
                           }),
                       );
                }
            }

            (
                StatusCode::OK,
                Json(UpdateMessageResponse {
                    response_message: "Message updated successfully".to_string(),
                    response: Some(updated_message),
                    error: None,
                }),
            )
        }
        Err(e) => {
            error!("FAILED TO UPDATE MESSAGE!");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpdateMessageResponse {
                    response_message: "Failed to update message".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}
