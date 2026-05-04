use sea_orm::*;

use crate::entity::{agent_task_events, agent_tasks, external_agents};
use crate::error::{AQBotError, Result};
use crate::types::{
    AgentTask, AgentTaskEvent, CreateExternalAgentInput, ExternalAgent, UpdateExternalAgentInput,
};
use crate::utils::{gen_id, now_ts};

fn external_agent_from_model(model: external_agents::Model) -> ExternalAgent {
    ExternalAgent {
        id: model.id,
        name: model.name,
        kind: model.kind,
        base_url: model.base_url,
        auth_type: model.auth_type,
        auth_config_json: model.auth_config_json,
        capabilities_json: model.capabilities_json,
        enabled: model.enabled != 0,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn agent_task_from_model(model: agent_tasks::Model) -> AgentTask {
    AgentTask {
        id: model.id,
        conversation_id: model.conversation_id,
        source_message_id: model.source_message_id,
        external_agent_id: model.external_agent_id,
        external_task_id: model.external_task_id,
        kind: model.kind,
        status: model.status,
        title: model.title,
        request_payload_json: model.request_payload_json,
        result_payload_json: model.result_payload_json,
        error_message: model.error_message,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn agent_task_event_from_model(model: agent_task_events::Model) -> AgentTaskEvent {
    AgentTaskEvent {
        id: model.id,
        task_id: model.task_id,
        event_type: model.event_type,
        payload_json: model.payload_json,
        created_at: model.created_at,
    }
}

pub async fn list_external_agents(db: &DatabaseConnection) -> Result<Vec<ExternalAgent>> {
    let rows = external_agents::Entity::find()
        .order_by_asc(external_agents::Column::Name)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(external_agent_from_model).collect())
}

pub async fn get_external_agent(db: &DatabaseConnection, id: &str) -> Result<ExternalAgent> {
    let row = external_agents::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("ExternalAgent {}", id)))?;
    Ok(external_agent_from_model(row))
}

pub async fn create_external_agent(
    db: &DatabaseConnection,
    input: CreateExternalAgentInput,
) -> Result<ExternalAgent> {
    if input.name.trim().is_empty() {
        return Err(AQBotError::Validation("Agent name is required".into()));
    }
    let id = gen_id();
    let now = now_ts();
    external_agents::ActiveModel {
        id: Set(id.clone()),
        name: Set(input.name.trim().to_string()),
        kind: Set(if input.kind.trim().is_empty() {
            "generic_http".to_string()
        } else {
            input.kind.trim().to_string()
        }),
        base_url: Set(input.base_url.filter(|value| !value.trim().is_empty())),
        auth_type: Set(input.auth_type.unwrap_or_else(|| "none".to_string())),
        auth_config_json: Set(input
            .auth_config_json
            .filter(|value| !value.trim().is_empty())),
        capabilities_json: Set(input
            .capabilities_json
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "{}".to_string())),
        enabled: Set(if input.enabled.unwrap_or(true) { 1 } else { 0 }),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;
    get_external_agent(db, &id).await
}

pub async fn update_external_agent(
    db: &DatabaseConnection,
    id: &str,
    input: UpdateExternalAgentInput,
) -> Result<ExternalAgent> {
    let row = external_agents::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("ExternalAgent {}", id)))?;
    let mut am: external_agents::ActiveModel = row.into();

    if let Some(name) = input.name {
        if name.trim().is_empty() {
            return Err(AQBotError::Validation("Agent name is required".into()));
        }
        am.name = Set(name.trim().to_string());
    }
    if let Some(kind) = input.kind {
        am.kind = Set(if kind.trim().is_empty() {
            "generic_http".to_string()
        } else {
            kind.trim().to_string()
        });
    }
    if let Some(base_url) = input.base_url {
        am.base_url = Set(base_url.filter(|value| !value.trim().is_empty()));
    }
    if let Some(auth_type) = input.auth_type {
        am.auth_type = Set(auth_type);
    }
    if let Some(auth_config_json) = input.auth_config_json {
        am.auth_config_json = Set(auth_config_json.filter(|value| !value.trim().is_empty()));
    }
    if let Some(capabilities_json) = input.capabilities_json {
        am.capabilities_json = Set(if capabilities_json.trim().is_empty() {
            "{}".to_string()
        } else {
            capabilities_json
        });
    }
    if let Some(enabled) = input.enabled {
        am.enabled = Set(if enabled { 1 } else { 0 });
    }
    am.updated_at = Set(now_ts());
    am.update(db).await?;
    get_external_agent(db, id).await
}

pub async fn delete_external_agent(db: &DatabaseConnection, id: &str) -> Result<()> {
    let result = external_agents::Entity::delete_by_id(id).exec(db).await?;
    if result.rows_affected == 0 {
        return Err(AQBotError::NotFound(format!("ExternalAgent {}", id)));
    }
    Ok(())
}

pub async fn create_agent_task(
    db: &DatabaseConnection,
    conversation_id: Option<&str>,
    source_message_id: Option<&str>,
    external_agent_id: &str,
    kind: &str,
    status: &str,
    title: &str,
    request_payload_json: &str,
) -> Result<AgentTask> {
    let id = gen_id();
    let now = now_ts();
    agent_tasks::ActiveModel {
        id: Set(id.clone()),
        conversation_id: Set(conversation_id.map(|value| value.to_string())),
        source_message_id: Set(source_message_id.map(|value| value.to_string())),
        external_agent_id: Set(external_agent_id.to_string()),
        external_task_id: Set(None),
        kind: Set(kind.to_string()),
        status: Set(status.to_string()),
        title: Set(title.to_string()),
        request_payload_json: Set(request_payload_json.to_string()),
        result_payload_json: Set(None),
        error_message: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;
    get_agent_task(db, &id).await
}

pub async fn get_agent_task(db: &DatabaseConnection, id: &str) -> Result<AgentTask> {
    let row = agent_tasks::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("AgentTask {}", id)))?;
    Ok(agent_task_from_model(row))
}

pub async fn update_agent_task_result(
    db: &DatabaseConnection,
    id: &str,
    status: &str,
    external_task_id: Option<String>,
    result_payload_json: Option<String>,
    error_message: Option<String>,
) -> Result<AgentTask> {
    let row = agent_tasks::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("AgentTask {}", id)))?;
    let mut am: agent_tasks::ActiveModel = row.into();
    am.status = Set(status.to_string());
    if external_task_id.is_some() {
        am.external_task_id = Set(external_task_id);
    }
    am.result_payload_json = Set(result_payload_json);
    am.error_message = Set(error_message);
    am.updated_at = Set(now_ts());
    am.update(db).await?;
    get_agent_task(db, id).await
}

pub async fn list_agent_tasks(
    db: &DatabaseConnection,
    conversation_id: Option<&str>,
    external_agent_id: Option<&str>,
    limit: Option<u64>,
) -> Result<Vec<AgentTask>> {
    let mut query = agent_tasks::Entity::find();
    if let Some(conversation_id) = conversation_id {
        query = query.filter(agent_tasks::Column::ConversationId.eq(conversation_id));
    }
    if let Some(external_agent_id) = external_agent_id {
        query = query.filter(agent_tasks::Column::ExternalAgentId.eq(external_agent_id));
    }
    let rows = query
        .order_by_desc(agent_tasks::Column::CreatedAt)
        .limit(limit.unwrap_or(100))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(agent_task_from_model).collect())
}

pub async fn create_agent_task_event(
    db: &DatabaseConnection,
    task_id: &str,
    event_type: &str,
    payload_json: &str,
) -> Result<AgentTaskEvent> {
    let id = gen_id();
    agent_task_events::ActiveModel {
        id: Set(id.clone()),
        task_id: Set(task_id.to_string()),
        event_type: Set(event_type.to_string()),
        payload_json: Set(payload_json.to_string()),
        created_at: Set(now_ts()),
    }
    .insert(db)
    .await?;
    let row = agent_task_events::Entity::find_by_id(&id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("AgentTaskEvent {}", id)))?;
    Ok(agent_task_event_from_model(row))
}

pub async fn list_agent_task_events(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Vec<AgentTaskEvent>> {
    let rows = agent_task_events::Entity::find()
        .filter(agent_task_events::Column::TaskId.eq(task_id))
        .order_by_asc(agent_task_events::Column::CreatedAt)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(agent_task_event_from_model).collect())
}
