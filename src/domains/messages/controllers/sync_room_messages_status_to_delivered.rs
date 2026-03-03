use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
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

pub async fn sync_room_messages_status_to_delivered(
    State(state): State<AppState>,
    Extension(_session): Extension<SessionsMiddlewareOutput>,
    // Query(params): Query<SearchParams>,
    Path(room_id): Path<i64>,
    Json(payload): Json<PayloadSpecs>,
) -> impl IntoResponse {
    let user_id = payload.user_id;

    // Step 1: Fetch all the messages in this room
    let messages_result = sqlx::query_as::<_, Message>(
        r#"
        SELECT * 
        FROM messages 
        WHERE room_id = $1 
        ORDER BY created_at ASC
        "#
    )
    .bind(room_id)
    .fetch_all(&state.db)
    .await;

    // get room
    let room_res = sqlx::query_as::<_, Room>(
        r#"
        SELECT * FROM rooms WHERE id = $1
        "#
    )
    .bind(&room_id)
    .fetch_one(&state.db)
    .await;

    let room = match room_res {
        Ok(r) => r,
        Err(e) => {
            error!("FAILED TO GET ROOM: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncRoomMessagesStatusResponse {
                    response_message: "Failed to get room".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    let mut receipt_res: Result<(), sqlx::Error>;

    match messages_result {
        Ok(messages) => {
            // Step 2: For each message, fetch the status receipts that match the latest update on the message
            for message in messages {
                if message.sender_id != Some(user_id) {
                    match room.is_group {
                        false => {
                            // Step 3: create a delivery receipt for this user
                            if user_id == room.co_member.unwrap() {
                                receipt_res = sqlx::query(
                                    r#"
                                    INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status)
                                    VALUES ($1, $2, $3, $4, 'system', 'delivered')
                                    "#
                                )
                                .bind(message.id)
                                .bind(message.sender_id)
                                .bind(user_id)
                                .bind(room_id)
                                .execute(&state.db)
                                .await
                                .map(|_| ());
    
                                if receipt_res.is_err() {
                                    error!("FAILED TO INSERT MESSAGE STATUS RECEIPT!");
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(SyncRoomMessagesStatusResponse {
                                            response_message: "Failed to insert message status receipt".to_string(),
                                            response: None,
                                            error: Some("Failed to insert message status receipt".to_string()),
                                        }),
                                    );
                                }
    
                                // Step 4: Update the message status to delivered
                                let update_res = sqlx::query(
                                    r#"
                                    UPDATE messages 
                                    SET status = 'delivered' 
                                    WHERE id = $1
                                    "#
                                )
                                .bind(message.id)
                                .execute(&state.db)
                                .await
                                .map(|_| ());
                
                                if update_res.is_err() {
                                    error!("FAILED TO UPDATE MESSAGE STATUS!");
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(SyncRoomMessagesStatusResponse {
                                            response_message: "Failed to update message status".to_string(),
                                            response: None,
                                            error: Some("Failed to update message status".to_string()),
                                        }),
                                    );
                                }
                            } 
                        },
                        true => {
                            // similarly, create a delivery status receipt for this user
                            if room.co_members.clone().unwrap().contains(&user_id) {
                                receipt_res = sqlx::query(
                                    r#"
                                    INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status)
                                    VALUES ($1, $2, $3, $4, 'system', 'delivered')
                                    "#
                                )
                                .bind(message.id)
                                .bind(message.sender_id)
                                .bind(user_id)
                                .bind(room_id)
                                .execute(&state.db)
                                .await
                                .map(|_| ());
    
                                 if let Err(e) = receipt_res {
                                    error!("FAILED TO INSERT MESSAGE STATUS RECEIPT!");
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(SyncRoomMessagesStatusResponse {
                                            response_message: "Failed to insert message status receipt".to_string(),
                                            response: None,
                                            error: Some(e.to_string()),
                                        }),
                                    );
                                }
    
                                // Unlike for private chats, don't update message to delivered directly since this user might not be the only room member
                            } 
                        }
                    }
                }

                /* 
                Step 3: Now loop through all the current delivered
                receipts(of the latest message update counter) for each message 
                to see if every member has a delivered receipt
                
                P.S: Still ensure that we are performing this action for users that are not the sender
                */
                if room.is_group == true && message.sender_id != Some(user_id) {
                    let message_status_receipts_result = sqlx::query_as::<_, MessageStatusReceipt>(
                            r#"
                            SELECT * 
                            FROM message_status_receipts 
                            WHERE room_id = $1 AND updates_count_tracker = $2 AND status = $3
                            ORDER BY created_at ASC
                            "#
                        )
                        .bind(room_id)
                        .bind(message.updates_counter)
                        .bind("delivered")
                        .fetch_all(&state.db)
                        .await;

                    if message_status_receipts_result.is_err() {
                        error!("FAILED TO FETCH MESSAGE STATUS RECEIPTS!");
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(SyncRoomMessagesStatusResponse {
                                response_message: "Failed to fetch message status receipts".to_string(),
                                response: None,
                                error: Some("Failed to fetch message status receipts".to_string()),
                            }),
                        );
                    }

                    // Collect all receiver_ids from receipts
                    let receipt_receiver_ids: Vec<i64> = message_status_receipts_result
                        .unwrap()
                        .iter()
                        .map(|receipt| receipt.receiver_id)
                        .collect();

                    // Check if every co_member has a receipt
                    // let is_delivery_complete = room.co_members.unwrap().iter().all(|member_id| {
                    let is_delivery_complete = room.co_members.as_ref().map_or(false, |members| {
                        members.iter().all(|member_id| {
                            receipt_receiver_ids.contains(member_id)
                        })
                    });

                    // Only update if all members have received it
                    if is_delivery_complete {
                        let update_res = sqlx::query(
                            r#"
                            UPDATE messages 
                            SET status = 'delivered' 
                            WHERE id = $1
                            "#
                        )
                        .bind(message.id)
                        .execute(&state.db)
                        .await
                        .map(|_| ());

                        if update_res.is_err() {
                            error!("FAILED TO UPDATE MESSAGE STATUS!");
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(SyncRoomMessagesStatusResponse {
                                    response_message: "Failed to update message status".to_string(),
                                    response: None,
                                    error: Some("Failed to update message status".to_string()),
                                }),
                            );
                        }
                    }
                }

            }

            return (
            StatusCode::OK,
            Json(SyncRoomMessagesStatusResponse {
                response_message: "Room messages statuses synced successfully".to_string(),
                response: Some("Room messages statuses synced successfully".to_string()),
                error: None,
            }))
        },
        Err(e) => {
            error!("FAILED TO SYNC ROOM MESSAGES STATUSES!");

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SyncRoomMessagesStatusResponse {
                    response_message: "Failed to sync room messages statuses".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };
}