use aqbot_core::error::Result;
use aqbot_core::file_store::FileStore;
use aqbot_core::types::{Attachment, Message, MessageRole};
use base64::Engine;
use sea_orm::DatabaseConnection;
use serde_json::Value;

fn artifact_field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value
        .get(key)
        .or_else(|| value.get("result").and_then(|result| result.get(key)))
}

async fn save_inline_artifacts(
    db: &DatabaseConnection,
    conversation_id: &str,
    payload: &Value,
) -> Result<Vec<Attachment>> {
    let Some(artifacts) = artifact_field(payload, "artifacts").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let file_store = FileStore::new();
    let mut attachments = Vec::new();

    for artifact in artifacts {
        let Some(name) = artifact.get("name").and_then(Value::as_str) else {
            continue;
        };
        let mime_type = artifact
            .get("mimeType")
            .or_else(|| artifact.get("mime_type"))
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream");
        let data_base64 = artifact
            .get("dataBase64")
            .or_else(|| artifact.get("data_base64"))
            .or_else(|| artifact.get("data"))
            .and_then(Value::as_str);
        let Some(data_base64) = data_base64 else {
            continue;
        };

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(data_base64.trim()))
            .map_err(|e| {
                aqbot_core::error::AQBotError::Validation(format!("Invalid artifact base64: {e}"))
            })?;
        let saved = file_store.save_file(&bytes, name, mime_type)?;
        let stored_file_id = aqbot_core::utils::gen_id();
        aqbot_core::repo::stored_file::create_stored_file(
            db,
            &stored_file_id,
            &saved.hash,
            name,
            mime_type,
            saved.size_bytes,
            &saved.storage_path,
            Some(conversation_id),
        )
        .await?;

        attachments.push(Attachment {
            id: stored_file_id,
            file_type: mime_type.to_string(),
            file_name: name.to_string(),
            file_path: saved.storage_path,
            file_size: saved.size_bytes as u64,
            data: None,
        });
    }

    Ok(attachments)
}

pub async fn ingest_assistant_message(
    db: &DatabaseConnection,
    conversation_id: &str,
    source_message_id: Option<&str>,
    content: &str,
    payload: &Value,
) -> Result<Message> {
    let attachments = save_inline_artifacts(db, conversation_id, payload).await?;
    aqbot_core::repo::message::create_message(
        db,
        conversation_id,
        MessageRole::Assistant,
        content,
        &attachments,
        source_message_id,
        0,
    )
    .await
}
