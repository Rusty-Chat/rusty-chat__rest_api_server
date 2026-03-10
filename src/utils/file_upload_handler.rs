use aws_sdk_s3::Client;
use axum::extract::State;
use axum::extract::multipart::{Field, MultipartError};
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct S3AppState {
    pub s3_client: Client,
    pub bucket_name: String,
    pub bucket_url: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
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
    #[serde(skip_serializing)]
    password: String,
    is_admin: bool,
    is_active: bool,
}

pub struct UpdateResponse {
    response_message: String,
    response: Option<UserProfile>,
    error: Option<String>,
}

pub enum UploadType {
    UserProfileImage,
    RoomProfileImage,
    MessageAttachment_1,
    MessageAttachment_2,
    MessageAttachment_3,
    MessageAttachment_4,
}

pub async fn upload_file(
    State(state): State<&crate::AppState>,
    field: Field<'_>,
    upload_id: &i64,
    upload_type: UploadType,
) -> Result<String, MultipartError> {
    // Your implementation

    // Generate unique S3 key
    let extension = field
        .file_name()
        .expect("Failed to extract object file name!")
        .split('.')
        .next_back()
        .expect("Failed to get object extension");

    let file_key = match upload_type {
        UploadType::RoomProfileImage => {
            format!("room_image_{}.{}", upload_id, extension)
        }
        UploadType::UserProfileImage => {
            format!("profile_image_{}.{}", upload_id, extension)
        }
        UploadType::MessageAttachment_1 => {
            format!(
                "message_attachment_1_{}.{}",
                upload_id,
                // uuid::Uuid::new_v4(),
                extension
            )
        }
        UploadType::MessageAttachment_2 => {
            format!(
                "message_attachment_2_{}.{}",
                upload_id,
                // uuid::Uuid::new_v4(),
                extension
            )
        }
        UploadType::MessageAttachment_3 => {
            format!(
                "message_attachment_3_{}.{}",
                upload_id,
                // uuid::Uuid::new_v4(),
                extension
            )
        }
        UploadType::MessageAttachment_4 => {
            format!(
                "message_attachment_4_{}.{}",
                upload_id,
                // uuid::Uuid::new_v4(),
                extension
            )
        }
    };

    let file_url = format!("{}/{}", state.s3.bucket_url.trim_end_matches('/'), file_key);

    let content_type = field
        .content_type()
        .map(|ct| ct.to_string())
        .expect("Failed to extract object content type!");

    // Use streaming body for S3 upload
    // let body = aws_sdk_s3::primitives::ByteStream::from_stream(field);
    let data = field.bytes().await?.to_vec();

    // Upload to S3
    state
        .s3
        .s3_client
        .put_object()
        .bucket(&state.s3.bucket_name)
        .key(&file_key)
        .content_type(content_type)
        .body(data.into())
        // .body(body)
        .send()
        .await
        .expect("S3 File upload failed!");

    Ok(file_url)
}

pub async fn upload_file_from_bytes(
    State(state): State<&crate::AppState>,
    bytes: Vec<u8>,
    filename: &str,
    upload_id: &i64,
    upload_type: UploadType,
) -> Result<String, String> {
    // Extract extension from filename
    let extension = filename.split('.').next_back().unwrap_or("bin");

    let file_key = match upload_type {
        UploadType::RoomProfileImage => {
            format!("room_image_{}.{}", upload_id, extension)
        }
        UploadType::UserProfileImage => {
            format!("profile_image_{}.{}", upload_id, extension)
        }
        UploadType::MessageAttachment_1 => {
            format!("message_attachment_1_{}.{}", upload_id, extension)
        }
        UploadType::MessageAttachment_2 => {
            format!("message_attachment_2_{}.{}", upload_id, extension)
        }
        UploadType::MessageAttachment_3 => {
            format!("message_attachment_3_{}.{}", upload_id, extension)
        }
        UploadType::MessageAttachment_4 => {
            format!("message_attachment_4_{}.{}", upload_id, extension)
        }
    };

    let file_url = format!("{}/{}", state.s3.bucket_url.trim_end_matches('/'), file_key);

    // Infer content type from extension
    let content_type = match extension {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    };

    let byte_stream = aws_sdk_s3::primitives::ByteStream::from(bytes);

    // Upload to S3
    state
        .s3
        .s3_client
        .put_object()
        .bucket(&state.s3.bucket_name)
        .key(&file_key)
        .content_type(content_type)
        // .body(bytes.into())
        .body(byte_stream)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(file_url)
}

// pub async fn streaming_upload(
//     State(state): State<&crate::AppState>,
//     field: Field<'_>,
//     upload_id: &str,
//     upload_type: UploadType,
// ) -> Result<String, MultipartError> {
//     // Extract extension from filename
//     let extension = field
//         .file_name()
//         .unwrap_or("unknown")
//         .split('.')
//         .next_back()
//         .unwrap_or("bin");

//     let file_key = match upload_type {
//         UploadType::RoomProfileImage => {
//             format!("room_image_{}.{}", upload_id, extension)
//         }
//         UploadType::UserProfileImage => {
//             format!("profile_image_{}.{}", upload_id, extension)
//         }
//         UploadType::MessageAttachment_1 => {
//             format!("message_attachment_1_{}.{}", upload_id, extension)
//         }
//         UploadType::MessageAttachment_2 => {
//             format!("message_attachment_2_{}.{}", upload_id, extension)
//         }
//         UploadType::MessageAttachment_3 => {
//             format!("message_attachment_3_{}.{}", upload_id, extension)
//         }
//         UploadType::MessageAttachment_4 => {
//             format!("message_attachment_4_{}.{}", upload_id, extension)
//         }
//     };

//     let aws_region = state.config.aws.as_ref().unwrap().s3_bucket_region.clone();

//     let file_url = format!(
//         "https://{}.s3.{}.amazonaws.com/{}",
//         state.s3.bucket_name, aws_region, file_key
//     );

//     let content_type = field
//         .content_type()
//         .map(|ct| ct.to_string())
//         .unwrap_or_else(|| {
//             // Infer from extension if not provided
//             match extension {
//                 "jpg" | "jpeg" => "image/jpeg".to_string(),
//                 "png" => "image/png".to_string(),
//                 "gif" => "image/gif".to_string(),
//                 "webp" => "image/webp".to_string(),
//                 "pdf" => "application/pdf".to_string(),
//                 "mp4" => "video/mp4".to_string(),
//                 "mp3" => "audio/mpeg".to_string(),
//                 _ => "application/octet-stream".to_string(),
//             }
//         });

//     // Use streaming body for S3 upload
//     let body = aws_sdk_s3::primitives::ByteStream::from_field(field);

//     // Upload to S3
//     state
//         .s3
//         .s3_client
//         .put_object()
//         .bucket(&state.s3.bucket_name)
//         .key(&file_key)
//         .content_type(content_type)
//         .body(body)
//         .send()
//         .await
//         .expect("S3 File upload failed!");

//     Ok(file_url)
// }
