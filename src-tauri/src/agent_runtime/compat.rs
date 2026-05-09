use aqbot_core::repo::{agent_session, conversation, message, provider};
use aqbot_core::types::{AgentProfile, AgentSession, MessageRole, ProviderType};
use aqbot_providers::ProviderAdapter;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tauri::Emitter;
use tokio::sync::RwLock;

pub static RUNNING_AGENTS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct RunningAgentGuard {
    pub conversation_id: String,
    pub run_id: String,
}

impl Drop for RunningAgentGuard {
    fn drop(&mut self) {
        if let Ok(mut running) = RUNNING_AGENTS.lock() {
            if running.get(&self.conversation_id) == Some(&self.run_id) {
                running.remove(&self.conversation_id);
            }
        }
    }
}

pub struct AgentCancelTokenGuard {
    pub conversation_id: String,
    pub tokens: Arc<tokio::sync::Mutex<HashMap<String, open_agent_sdk::CancellationToken>>>,
}

impl Drop for AgentCancelTokenGuard {
    fn drop(&mut self) {
        let conversation_id = self.conversation_id.clone();
        let tokens = self.tokens.clone();
        tokio::spawn(async move {
            tokens.lock().await.remove(&conversation_id);
        });
    }
}

pub async fn ensure_agent_assistant_message(
    db: &sea_orm::DatabaseConnection,
    app: &tauri::AppHandle,
    conv_id: &str,
    user_msg_id: &str,
    assistant_created_at: i64,
    content: &str,
    current_assistant_msg_id: &mut Option<String>,
    assistant_id_for_task: &Arc<RwLock<Option<String>>>,
) -> Option<String> {
    if let Some(message_id) = current_assistant_msg_id.clone() {
        return Some(message_id);
    }

    match message::create_message_with_created_at(
        db,
        conv_id,
        MessageRole::Assistant,
        content,
        &[],
        Some(user_msg_id),
        0,
        assistant_created_at,
    )
    .await
    {
        Ok(assist_msg) => {
            let message_id = assist_msg.id.clone();
            *current_assistant_msg_id = Some(message_id.clone());
            *assistant_id_for_task.write().await = Some(message_id.clone());
            let _ = app.emit(
                "agent-message-id",
                serde_json::json!({
                    "conversationId": conv_id,
                    "assistantMessageId": message_id.clone(),
                }),
            );
            let _ = conversation::increment_message_count(db, conv_id).await;
            Some(message_id)
        }
        Err(err) => {
            tracing::warn!("[agent] Failed to create assistant message: {}", err);
            None
        }
    }
}

pub async fn persist_agent_partial_content(
    db: &sea_orm::DatabaseConnection,
    app: &tauri::AppHandle,
    conv_id: &str,
    user_msg_id: &str,
    assistant_created_at: i64,
    content: &str,
    current_assistant_msg_id: &mut Option<String>,
    assistant_id_for_task: &Arc<RwLock<Option<String>>>,
) -> Option<String> {
    let message_id = ensure_agent_assistant_message(
        db,
        app,
        conv_id,
        user_msg_id,
        assistant_created_at,
        content,
        current_assistant_msg_id,
        assistant_id_for_task,
    )
    .await?;
    let _ = message::update_message_content(db, &message_id, content).await;
    Some(message_id)
}

pub fn provider_type_to_registry_key(pt: &ProviderType) -> &'static str {
    match pt {
        ProviderType::OpenAI => "openai",
        ProviderType::OpenAIResponses => "openai_responses",
        ProviderType::DeepSeek => "deepseek",
        ProviderType::XAI => "xai",
        ProviderType::GLM => "glm",
        ProviderType::SiliconFlow => "siliconflow",
        ProviderType::Anthropic => "anthropic",
        ProviderType::Gemini => "gemini",
        ProviderType::Jina => "jina",
        ProviderType::Cohere => "cohere",
        ProviderType::Voyage => "voyage",
        ProviderType::Custom => "custom",
    }
}

pub fn create_adapter_arc(pt: &ProviderType) -> Result<Arc<dyn ProviderAdapter>, String> {
    match pt {
        ProviderType::OpenAI => Ok(Arc::new(aqbot_providers::openai::OpenAIAdapter::new())),
        ProviderType::Custom => Ok(Arc::new(
            aqbot_providers::custom_openai::CustomOpenAIAdapter::new(),
        )),
        ProviderType::DeepSeek => Ok(Arc::new(aqbot_providers::deepseek::DeepSeekAdapter::new())),
        ProviderType::XAI => Ok(Arc::new(aqbot_providers::xai::XAIAdapter::new())),
        ProviderType::GLM => Ok(Arc::new(aqbot_providers::glm::GLMAdapter::new())),
        ProviderType::SiliconFlow => Ok(Arc::new(
            aqbot_providers::siliconflow::SiliconFlowAdapter::new(),
        )),
        ProviderType::Anthropic => {
            Ok(Arc::new(aqbot_providers::anthropic::AnthropicAdapter::new()))
        }
        ProviderType::Gemini => Ok(Arc::new(aqbot_providers::gemini::GeminiAdapter::new())),
        ProviderType::OpenAIResponses => Ok(Arc::new(
            aqbot_providers::openai_responses::OpenAIResponsesAdapter::new(),
        )),
        ProviderType::Jina | ProviderType::Cohere | ProviderType::Voyage => {
            Err("Rerank-only providers cannot be used as agent chat providers".to_string())
        }
    }
}

pub fn truncate_preview(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let cutoff = s
        .char_indices()
        .map(|(idx, _)| idx)
        .take_while(|&idx| idx <= max_len)
        .last()
        .unwrap_or(0);

    format!("{}...", &s[..cutoff])
}

pub fn get_tool_input_summary(tool_name: &str, input: &Value) -> String {
    let try_key = |key: &str| {
        input
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    if let Some(cmd) = try_key("command") {
        return cmd.chars().take(80).collect();
    }
    if let Some(path) = try_key("path").or_else(|| try_key("file_path")) {
        return path;
    }
    if let Some(pattern) = try_key("pattern") {
        return pattern.chars().take(80).collect();
    }
    if let Some(query) = try_key("query") {
        return query.chars().take(80).collect();
    }
    if let Some(content) = try_key("content") {
        return content.chars().take(60).collect();
    }
    if let Some(obj) = input.as_object() {
        for val in obj.values() {
            if let Some(s) = val.as_str() {
                return s.chars().take(80).collect();
            }
        }
    }
    tool_name.to_string()
}

pub async fn resolve_agent_provider_id(
    db: &sea_orm::DatabaseConnection,
    provider_id: &str,
) -> Result<String, String> {
    provider::resolve_provider_id(db, provider_id)
        .await
        .map_err(|e| e.to_string())
}

pub async fn ensure_legacy_session_for_profile(
    db: &sea_orm::DatabaseConnection,
    profile: &AgentProfile,
) -> Result<AgentSession, String> {
    agent_session::upsert_agent_session(
        db,
        &profile.conversation_id,
        profile.workspace_root.as_deref(),
        Some(&profile.permission_mode),
    )
    .await
    .map_err(|e| e.to_string())
}
