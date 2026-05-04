use aqbot_core::types::*;
use serde_json::json;
use tauri::State;

use crate::external_agents::context::collect_context as collect_external_context;
use crate::external_agents::custom_http::parse_json_object;
use crate::external_agents::registry::{
    dispatch_task as dispatch_connector_task,
    fetch_task as fetch_connector_task,
    test_connection as test_connector_connection,
};
use crate::external_agents::result_ingest::ingest_assistant_message;
use crate::AppState;

async fn dispatch_task_from_parts(
    state: &AppState,
    agent: &ExternalAgent,
    conversation_id: Option<&str>,
    source_message_id: Option<&str>,
    kind: &str,
    title: &str,
    input_text: &str,
    context: serde_json::Value,
) -> Result<DispatchExternalAgentTaskResult, String> {
    let capabilities = parse_json_object(Some(&agent.capabilities_json), json!({}))?;
    let request_payload = json!({
        "kind": kind,
        "title": title,
        "inputText": input_text,
        "context": context,
        "capabilities": capabilities,
    });
    let request_payload_json =
        serde_json::to_string(&request_payload).map_err(|e| e.to_string())?;
    let task = aqbot_core::repo::external_agent::create_agent_task(
        &state.sea_db,
        conversation_id,
        source_message_id,
        &agent.id,
        kind,
        "dispatching",
        title,
        &request_payload_json,
    )
    .await
    .map_err(|e| e.to_string())?;
    let task_payload = json!({
        "id": task.id,
        "conversationId": conversation_id,
        "sourceMessageId": source_message_id,
        "kind": kind,
        "title": title,
        "input": { "text": input_text },
        "context": request_payload["context"].clone(),
        "createdAt": task.created_at,
    });

    let _ = aqbot_core::repo::external_agent::create_agent_task_event(
        &state.sea_db,
        &task.id,
        "created",
        &request_payload_json,
    )
    .await;

    match dispatch_connector_task(agent, &task, task_payload).await {
        Ok(response) => {
            let result_payload_json =
                serde_json::to_string(&response.result_payload).map_err(|e| e.to_string())?;
            let updated_task = aqbot_core::repo::external_agent::update_agent_task_result(
                &state.sea_db,
                &task.id,
                &response.status,
                response.external_task_id,
                Some(result_payload_json.clone()),
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
            let _ = aqbot_core::repo::external_agent::create_agent_task_event(
                &state.sea_db,
                &task.id,
                &response.status,
                &result_payload_json,
            )
            .await;

            let assistant_message = match (conversation_id, response.assistant_content.as_deref()) {
                (Some(conversation_id), Some(content)) if !content.trim().is_empty() => {
                    Some(
                        ingest_assistant_message(
                            &state.sea_db,
                            conversation_id,
                            source_message_id,
                            content,
                            &response.result_payload,
                        )
                        .await
                        .map_err(|e| e.to_string())?,
                    )
                }
                _ => None,
            };

            Ok(DispatchExternalAgentTaskResult {
                task: updated_task,
                assistant_message,
            })
        }
        Err(error) => {
            let updated_task = aqbot_core::repo::external_agent::update_agent_task_result(
                &state.sea_db,
                &task.id,
                "failed",
                None,
                None,
                Some(error.clone()),
            )
            .await
            .map_err(|e| e.to_string())?;
            let _ = aqbot_core::repo::external_agent::create_agent_task_event(
                &state.sea_db,
                &task.id,
                "failed",
                &json!({ "error": error }).to_string(),
            )
            .await;
            Ok(DispatchExternalAgentTaskResult {
                task: updated_task,
                assistant_message: None,
            })
        }
    }
}

#[tauri::command]
pub async fn list_external_agents(state: State<'_, AppState>) -> Result<Vec<ExternalAgent>, String> {
    aqbot_core::repo::external_agent::list_external_agents(&state.sea_db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_external_agent(
    state: State<'_, AppState>,
    input: CreateExternalAgentInput,
) -> Result<ExternalAgent, String> {
    parse_json_object(input.capabilities_json.as_deref(), json!({}))?;
    if input
        .auth_config_json
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        parse_json_object(input.auth_config_json.as_deref(), json!({}))?;
    }
    aqbot_core::repo::external_agent::create_external_agent(&state.sea_db, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_external_agent(
    state: State<'_, AppState>,
    id: String,
    input: UpdateExternalAgentInput,
) -> Result<ExternalAgent, String> {
    if let Some(raw) = input.capabilities_json.as_deref() {
        parse_json_object(Some(raw), json!({}))?;
    }
    if let Some(Some(raw)) = input.auth_config_json.as_ref() {
        if !raw.trim().is_empty() {
            parse_json_object(Some(raw), json!({}))?;
        }
    }
    aqbot_core::repo::external_agent::update_external_agent(&state.sea_db, &id, input)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_external_agent(state: State<'_, AppState>, id: String) -> Result<(), String> {
    aqbot_core::repo::external_agent::delete_external_agent(&state.sea_db, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_agent_tasks(
    state: State<'_, AppState>,
    conversation_id: Option<String>,
    external_agent_id: Option<String>,
    limit: Option<u64>,
) -> Result<Vec<AgentTask>, String> {
    aqbot_core::repo::external_agent::list_agent_tasks(
        &state.sea_db,
        conversation_id.as_deref(),
        external_agent_id.as_deref(),
        limit,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_agent_task_events(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<AgentTaskEvent>, String> {
    aqbot_core::repo::external_agent::list_agent_task_events(&state.sea_db, &task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_external_agent_connection(
    state: State<'_, AppState>,
    id: String,
) -> Result<ExternalAgentConnectionTestResult, String> {
    let agent = aqbot_core::repo::external_agent::get_external_agent(&state.sea_db, &id)
        .await
        .map_err(|e| e.to_string())?;
    test_connector_connection(&agent).await
}

#[tauri::command]
pub async fn dispatch_external_agent_task(
    state: State<'_, AppState>,
    input: DispatchExternalAgentTaskInput,
) -> Result<DispatchExternalAgentTaskResult, String> {
    if input.external_agent_id.trim().is_empty() {
        return Err("externalAgentId is required".to_string());
    }
    if input.title.trim().is_empty() {
        return Err("title is required".to_string());
    }

    let agent =
        aqbot_core::repo::external_agent::get_external_agent(&state.sea_db, &input.external_agent_id)
            .await
            .map_err(|e| e.to_string())?;
    let parsed_context = parse_json_object(input.context_json.as_deref(), json!({}))?;
    let mut context = if parsed_context.is_object() {
        parsed_context
    } else {
        json!({ "inputContext": parsed_context })
    };
    let auto_context = collect_external_context(&state, &input.input_text).await;
    if !auto_context
        .as_object()
        .map(|value| value.is_empty())
        .unwrap_or(true)
    {
        context["aqbotContext"] = auto_context;
    }

    dispatch_task_from_parts(
        &state,
        &agent,
        input.conversation_id.as_deref(),
        input.source_message_id.as_deref(),
        input.kind.as_deref().unwrap_or("general"),
        &input.title,
        &input.input_text,
        context,
    )
    .await
}

#[tauri::command]
pub async fn retry_external_agent_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<DispatchExternalAgentTaskResult, String> {
    let task = aqbot_core::repo::external_agent::get_agent_task(&state.sea_db, &task_id)
        .await
        .map_err(|e| e.to_string())?;
    let agent = aqbot_core::repo::external_agent::get_external_agent(
        &state.sea_db,
        &task.external_agent_id,
    )
    .await
    .map_err(|e| e.to_string())?;
    let request_payload = parse_json_object(Some(&task.request_payload_json), json!({}))?;
    let kind = request_payload
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or(task.kind.as_str())
        .to_string();
    let title = request_payload
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or(task.title.as_str())
        .to_string();
    let input_text = request_payload
        .get("inputText")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "request payload is missing inputText".to_string())?
        .to_string();
    let context = request_payload
        .get("context")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let _ = aqbot_core::repo::external_agent::create_agent_task_event(
        &state.sea_db,
        &task.id,
        "retry_requested",
        &json!({
            "sourceTaskId": task.id,
            "status": task.status,
        })
        .to_string(),
    )
    .await;

    let result = dispatch_task_from_parts(
        &state,
        &agent,
        task.conversation_id.as_deref(),
        task.source_message_id.as_deref(),
        &kind,
        &title,
        &input_text,
        context,
    )
    .await?;

    let _ = aqbot_core::repo::external_agent::create_agent_task_event(
        &state.sea_db,
        &task.id,
        "retried",
        &json!({
            "newTaskId": result.task.id,
            "newStatus": result.task.status,
        })
        .to_string(),
    )
    .await;

    Ok(result)
}

#[tauri::command]
pub async fn sync_external_agent_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<DispatchExternalAgentTaskResult, String> {
    let task = aqbot_core::repo::external_agent::get_agent_task(&state.sea_db, &task_id)
        .await
        .map_err(|e| e.to_string())?;
    let agent = aqbot_core::repo::external_agent::get_external_agent(
        &state.sea_db,
        &task.external_agent_id,
    )
    .await
    .map_err(|e| e.to_string())?;
    let external_task_id = task
        .external_task_id
        .clone()
        .unwrap_or_else(|| task.id.clone());

    let _ = aqbot_core::repo::external_agent::create_agent_task_event(
        &state.sea_db,
        &task.id,
        "sync_requested",
        &json!({ "externalTaskId": external_task_id }).to_string(),
    )
    .await;

    let response = fetch_connector_task(&agent, &external_task_id)
        .await
        .map_err(|e| e.to_string())?;
    let result_payload_json =
        serde_json::to_string(&response.result_payload).map_err(|e| e.to_string())?;
    let updated_task = aqbot_core::repo::external_agent::update_agent_task_result(
        &state.sea_db,
        &task.id,
        &response.status,
        response.external_task_id.clone(),
        Some(result_payload_json.clone()),
        None,
    )
    .await
    .map_err(|e| e.to_string())?;

    let _ = aqbot_core::repo::external_agent::create_agent_task_event(
        &state.sea_db,
        &task.id,
        &response.status,
        &result_payload_json,
    )
    .await;

    let task_events =
        aqbot_core::repo::external_agent::list_agent_task_events(&state.sea_db, &task.id)
            .await
            .map_err(|e| e.to_string())?;
    let already_ingested = task_events
        .iter()
        .any(|event| event.event_type == "ingested");

    let assistant_message = match (
        updated_task.conversation_id.as_deref(),
        updated_task.source_message_id.as_deref(),
        response.assistant_content.as_deref(),
        updated_task.status.as_str(),
        already_ingested,
    ) {
        (Some(conversation_id), source_message_id, Some(content), "completed", false)
            if !content.trim().is_empty() =>
        {
            let message = ingest_assistant_message(
                &state.sea_db,
                conversation_id,
                source_message_id,
                content,
                &response.result_payload,
            )
            .await
            .map_err(|e| e.to_string())?;
            let _ = aqbot_core::repo::external_agent::create_agent_task_event(
                &state.sea_db,
                &task.id,
                "ingested",
                &json!({ "messageId": message.id }).to_string(),
            )
            .await;
            Some(message)
        }
        _ => None,
    };

    Ok(DispatchExternalAgentTaskResult {
        task: updated_task,
        assistant_message,
    })
}
