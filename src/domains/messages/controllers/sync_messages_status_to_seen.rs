use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    Json,
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::error;

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
pub struct MessageStatusReceipt {
    pub id: i64,
    pub message_id: i64,
    pub sender_id: i64,
    pub receiver_id: i64,
    pub room_id: i64,
    pub status: String,
    pub action: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub updates_count_tracker: i32,
}

#[derive(Debug, Serialize)]
pub struct SyncRoomMessagesStatusResponse {
    pub response_message: String,
    pub response: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct PayloadSpecs {
    user_id: i64,
}

pub async fn sync_messages_status_to_seen(
    State(state): State<AppState>,
    Extension(_session): Extension<SessionsMiddlewareOutput>,
    Json(payload): Json<PayloadSpecs>,
) -> impl IntoResponse {
    let user_id = payload.user_id;

    // Step 1: Fetch all rooms the user is part of
    let rooms_res = sqlx::query_as::<_, Room>(
        r#"
        SELECT * FROM rooms 
        WHERE co_member = $1 OR $1 = ANY(co_members)
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await;

    let rooms = match rooms_res {
        Ok(r) => r,
        Err(e) => {
            error!("FAILED TO GET ROOMS: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncRoomMessagesStatusResponse {
                    response_message: "Failed to get rooms".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    for room in rooms {
        let room_id = room.id;

        // Step 2: Fetch all the messages in this room (excluding those already 'seen' if we want optimization,
        // but following the original pattern of fetching all and checking)
        let messages_result = sqlx::query_as::<_, Message>(
            r#"
            SELECT * 
            FROM messages 
            WHERE room_id = $1 AND status NOT IN ('seen')
            ORDER BY created_at ASC
            "#,
        )
        .bind(room_id)
        .fetch_all(&state.db)
        .await;

        let messages = match messages_result {
            Ok(m) => m,
            Err(e) => {
                error!("FAILED TO GET MESSAGES FOR ROOM {}: {}", room_id, e);
                continue; // Skip this room if messages fail
            }
        };

        for message in messages {
            // Only create seen receipt if the user is NOT the sender
            if message.sender_id != Some(user_id) {
                match room.is_group {
                    false => {
                        // For private rooms, if current user is the co_member (receiver)
                        if Some(user_id) == room.co_member {
                            let receipt_res = sqlx::query(
                                r#"
                                INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status)
                                VALUES ($1, $2, $3, $4, 'system', 'seen')
                                ON CONFLICT DO NOTHING
                                "#
                            )
                            .bind(message.id)
                            .bind(message.sender_id)
                            .bind(user_id)
                            .bind(room_id)
                            .execute(&state.db)
                            .await;

                            if let Ok(_) = receipt_res {
                                // Update the message status to seen
                                let _ = sqlx::query(
                                    r#"
                                    UPDATE messages 
                                    SET status = 'seen' 
                                    WHERE id = $1
                                    "#,
                                )
                                .bind(message.id)
                                .execute(&state.db)
                                .await;
                            }
                        }
                    }
                    true => {
                        // For group rooms, if user is in co_members
                        if room
                            .co_members
                            .as_ref()
                            .map_or(false, |members| members.contains(&user_id))
                        {
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status)
                                VALUES ($1, $2, $3, $4, 'system', 'seen')
                                ON CONFLICT DO NOTHING
                                "#
                            )
                            .bind(message.id)
                            .bind(message.sender_id)
                            .bind(user_id)
                            .bind(room_id)
                            .execute(&state.db)
                            .await;

                            // Now check if EVERY member (except sender) has a seen receipt
                            let receipts_res = sqlx::query_as::<_, MessageStatusReceipt>(
                                r#"
                                SELECT * 
                                FROM message_status_receipts 
                                WHERE message_id = $1 AND status = 'seen' AND updates_count_tracker = $2
                                "#
                            )
                            .bind(message.id)
                            .bind(message.updates_counter)
                            .fetch_all(&state.db)
                            .await;

                            if let Ok(receipts) = receipts_res {
                                let receipt_receiver_ids: Vec<i64> =
                                    receipts.iter().map(|r| r.receiver_id).collect();

                                let is_seen_by_all =
                                    room.co_members.as_ref().map_or(false, |members| {
                                        members.iter().all(|member_id| {
                                            // If this member is the sender, they don't need a seen receipt
                                            if Some(*member_id) == message.sender_id {
                                                return true;
                                            }
                                            receipt_receiver_ids.contains(member_id)
                                        })
                                    });

                                println!("is_seen_by_all: {}", is_seen_by_all);

                                if is_seen_by_all {
                                    let _ = sqlx::query(
                                        r#"
                                        UPDATE messages 
                                        SET status = 'seen' 
                                        WHERE id = $1
                                        "#,
                                    )
                                    .bind(message.id)
                                    .execute(&state.db)
                                    .await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(SyncRoomMessagesStatusResponse {
            response_message: "All room messages statuses synced successfully".to_string(),
            response: Some("All room messages statuses synced successfully".to_string()),
            error: None,
        }),
    )
}
