use crate::AppState;
use crate::middlewares::auth_sessions_middleware::SessionsMiddlewareOutput;
use crate::utils::current_time_in_milliseconds;
use crate::utils::file_upload_handler::{UploadType, upload_file_from_bytes};
use axum::{
    extract::{Extension, Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::postgres::PgQueryResult;
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
pub struct CreateMessageResponse {
    pub response_message: String,
    pub response: Option<Message>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RoomMember {
    pub id: i64,
    pub room_id: i64,
    pub user_id: i64,
    pub role: String,
    pub joined_at: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

pub async fn create_message(
    State(state): State<AppState>,
    Extension(_session): Extension<SessionsMiddlewareOutput>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut room_id: Option<i64> = None;
    let mut sender_id: Option<i64> = None;
    let mut message_type: Option<String> = None;
    let mut text_content: Option<String> = None;
    
    // Store attachment data for later upload
    let mut attachments: Vec<(String, Vec<u8>, String)> = Vec::new(); // (field_name, bytes, filename)

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "room_id" => {
                let val = field.text().await;
                let text = match val {
                    Ok(id) => id,
                    Err(_) => {
                        error!("FAILED TO GET ROOM ID!");
                        
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(CreateMessageResponse {
                                response_message: "Room ID is required".to_string(),
                                response: None,
                                error: Some("Missing field: room_id".to_string()),
                            }),
                        );
                    }
                };

                let parsed = text.parse::<i64>();
                if let Ok(id) = parsed {
                    room_id = Some(id);
                } else {
                    error!("FAILED TO PARSE ROOM ID!");

                    return (
                        StatusCode::BAD_REQUEST,
                        Json(CreateMessageResponse {
                            response_message: "Room ID is required".to_string(),
                            response: None,
                            error: Some("Invalid room_id".to_string()),
                        }),
                    );
                }
            }
            "sender_id" => {
                let val = field.text().await;
                let text = match val {
                    Ok(id) => id,
                    Err(_) => {
                        error!("FAILED TO GET SENDER ID!");
                        
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(CreateMessageResponse {
                                response_message: "Sender ID is required".to_string(),
                                response: None,
                                error: Some("Missing field: sender_id".to_string()),
                            }),
                        );
                    }
                };

                let parsed = text.parse::<i64>();
                let id = match parsed {
                    Ok(id) => id,
                    Err(_) => {
                        error!("FAILED TO PARSE SENDER ID!");
                        
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(CreateMessageResponse {
                                response_message: "Sender ID is required".to_string(),
                                response: None,
                                error: Some("Invalid sender_id".to_string()),
                            }),
                        );
                    }
                };

                sender_id = Some(id);

                // Verify user is a member of the room
                let membership = sqlx::query_as::<_, RoomMember>(
                    r#"
                    SELECT *
                    FROM room_members
                    WHERE room_id = $1 AND user_id = $2
                    "#,
                )
                .bind(room_id)
                .bind(&sender_id)
                .fetch_optional(&state.db)
                .await;

                match membership {
                    Ok(Some(_)) => (),
                    Ok(None) => {
                        error!("SENDER IS NOT A MEMBER OF THIS ROOM!");
                        
                        return (
                            StatusCode::FORBIDDEN,
                            Json(CreateMessageResponse {
                                response_message: "Sender is not a member of this room".to_string(),
                                response: None,
                                error: Some("Forbidden".to_string()),
                            }),
                        );
                    }
                    Err(e) => {
                        error!("FAILED TO VERIFY ROOM MEMBERSHIP!");
                        
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(CreateMessageResponse {
                                response_message: "Failed to verify room membership".to_string(),
                                response: None,
                                error: Some(e.to_string()),
                            }),
                        );
                    }
                }
            }
            "type" => {
                if let Ok(val) = field.text().await {
                    message_type = Some(val);
                }
            }
            "text_content" => {
                text_content = field.text().await.ok();
            }
            "attachment_1" | "attachment_2" | "attachment_3" | "attachment_4" => {
                let filename = field.file_name().unwrap_or("unknown").to_string();
                if let Ok(bytes) = field.bytes().await {
                    attachments.push((name.clone(), bytes.to_vec(), filename));
                }
            }
            _ => {}
        }
    }

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
                Json(CreateMessageResponse {
                    response_message: "Failed to get room".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    let sent_at = current_time_in_milliseconds::current_time_millis();

    // Create message without attachments
    let res = sqlx::query_as::<_, Message>(
        r#"
        INSERT INTO messages (room_id, sender_id, type, text_content, attachment_1, attachment_2, attachment_3, attachment_4, status, sent_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *
        "#
    )
    .bind(&room_id)
    .bind(&sender_id)
    .bind(&message_type)
    .bind(&text_content)
    .bind("")
    .bind("")
    .bind("")
    .bind("")
    .bind("sent".to_string())
    .bind(sent_at.to_string())
    .fetch_one(&state.db)
    .await;

    let message = match res {
        Ok(msg) => msg,
        Err(e) => {
            error!("FAILED TO CREATE MESSAGE!");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateMessageResponse {
                    response_message: "Failed to create message".to_string(),
                    response: None,
                    error: Some(e.to_string()),
                }),
            );
        }
    };

    // Now upload attachments using the message ID
    let mut attachment_1: Option<String> = None;
    let mut attachment_2: Option<String> = None;
    let mut attachment_3: Option<String> = None;
    let mut attachment_4: Option<String> = None;

    // In the attachment processing loop, replace the placeholder with:
    for (field_name, bytes, filename) in attachments {
        let upload_type = match field_name.as_str() {
            "attachment_1" => UploadType::MessageAttachment1,
            "attachment_2" => UploadType::MessageAttachment2,
            "attachment_3" => UploadType::MessageAttachment3,
            "attachment_4" => UploadType::MessageAttachment4,
            _ => continue,
        };

        if let Ok(url) = upload_file_from_bytes(
            State(&state),
            bytes,
            &filename,
            &message.id,
            upload_type,
        )
        .await
        {
            match field_name.as_str() {
                "attachment_1" => attachment_1 = Some(url),
                "attachment_2" => attachment_2 = Some(url),
                "attachment_3" => attachment_3 = Some(url),
                "attachment_4" => attachment_4 = Some(url),
                _ => {}
            }
        }
    }

    // Update message with attachments
    let update_res = sqlx::query_as::<_, Message>(
        r#"
        UPDATE messages 
        SET attachment_1 = $1, attachment_2 = $2, attachment_3 = $3, attachment_4 = $4
        WHERE id = $5
        RETURNING *
        "#
    )
    .bind(&attachment_1)
    .bind(&attachment_2)
    .bind(&attachment_3)
    .bind(&attachment_4)
    .bind(&message.id)
    .fetch_one(&state.db)
    .await;

    let receipt_res: Result<(), sqlx::Error>;

    match update_res {
        Ok(msg) => {
            // Create "sent" status receipt(s)
            match room.is_group {
                false => {
                    receipt_res = sqlx::query(
                        r#"
                        INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status)
                        VALUES ($1, $2, $3, $4, 'original-send', 'sent')
                        "#
                    )
                    .bind(msg.id)
                    .bind(sender_id)
                    .bind(room.co_member)
                    .bind(room_id)
                    .execute(&state.db)
                    .await
                    .map(|_| ());
                },
                true => {
                    let mut temp_res = Ok(());
                    if let Some(members) = &room.co_members {
                        for room_member in members {
                             let res = sqlx::query(
                                r#"
                                INSERT INTO message_status_receipts (message_id, sender_id, receiver_id, room_id, action, status)
                                VALUES ($1, $2, $3, $4, 'original-send', 'sent')
                                "#
                            )
                            .bind(msg.id)
                            .bind(sender_id)
                            .bind(room_member)
                            .bind(room_id)
                            .execute(&state.db)
                            .await;

                            if let Err(e) = res {
                                temp_res = Err(e);
                                break;
                            }
                        }
                    }
                    receipt_res = temp_res;
                },
            }

            match receipt_res {
                Ok(_) => {
                    
                }, 
                Err(_e) => {
                    {
                       error!("MESSAGE CREATED SUCCESSFULLY, BUT FAILED TO CREATE MESSAGE STATUS RECEIPT!");
                       
                       return (
                           StatusCode::CREATED,
                           Json(CreateMessageResponse {
                               response_message: "Message created successfully but failed to create message status receipt".to_string(),
                               response: Some(msg),
                               error: None,
                           }),
                       )
                   }    
                }
            }
    
            (
                StatusCode::CREATED,
                Json(CreateMessageResponse {
                    response_message: "Message created successfully".to_string(),
                    response: Some(msg),
                    error: None,
                }),
            )
        },
        Err(e) => {
            error!("MESSAGE CREATED, BUT FAILED TO UPLOAD ATTACHMENTS!");
            // Return the message anyway since it was created, just without attachments
            (
            StatusCode::CREATED,
            Json(CreateMessageResponse {
                response_message: "Message created but failed to add attachments".to_string(),
                    response: Some(message),
                error: Some(e.to_string()),
            }),
        )
        }
    }
}