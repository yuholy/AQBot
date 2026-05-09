use crate::entity::{agent_profiles, agent_sessions};
use crate::error::Result;
use crate::types::{AgentProfile, AgentSession};
use crate::utils::gen_id;
use sea_orm::*;

fn now_string() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn model_to_agent_profile(model: agent_profiles::Model) -> AgentProfile {
    AgentProfile {
        id: model.id,
        conversation_id: model.conversation_id,
        workspace_root: model.workspace_root,
        permission_mode: model.permission_mode,
        default_runner_kind: model.default_runner_kind,
        default_provider_id: model.default_provider_id,
        default_model_id: model.default_model_id,
        created_at: model.created_at,
        updated_at: model.updated_at,
    }
}

fn default_workspace_root(conversation_id: &str) -> String {
    let absolute = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".aqbot")
        .join("workspace")
        .join(conversation_id);
    crate::path_vars::encode_path(&absolute.to_string_lossy())
}

pub async fn get_profile_by_conversation_id(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<Option<AgentProfile>> {
    let model = agent_profiles::Entity::find()
        .filter(agent_profiles::Column::ConversationId.eq(conversation_id))
        .one(db)
        .await?;
    Ok(model.map(model_to_agent_profile))
}

pub async fn list_profiles(db: &DatabaseConnection) -> Result<Vec<AgentProfile>> {
    let rows = agent_profiles::Entity::find()
        .order_by_desc(agent_profiles::Column::UpdatedAt)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(model_to_agent_profile).collect())
}

pub async fn upsert_profile(
    db: &DatabaseConnection,
    conversation_id: &str,
    workspace_root: Option<&str>,
    permission_mode: Option<&str>,
    default_runner_kind: Option<&str>,
    default_provider_id: Option<&str>,
    default_model_id: Option<&str>,
) -> Result<AgentProfile> {
    let existing = agent_profiles::Entity::find()
        .filter(agent_profiles::Column::ConversationId.eq(conversation_id))
        .one(db)
        .await?;
    let now = now_string();

    if let Some(model) = existing {
        let mut am: agent_profiles::ActiveModel = model.into();
        if let Some(value) = workspace_root {
            am.workspace_root = Set(Some(value.to_string()));
        }
        if let Some(value) = permission_mode {
            am.permission_mode = Set(value.to_string());
        }
        if let Some(value) = default_runner_kind {
            am.default_runner_kind = Set(value.to_string());
        }
        if let Some(value) = default_provider_id {
            am.default_provider_id = Set(Some(value.to_string()));
        }
        if let Some(value) = default_model_id {
            am.default_model_id = Set(Some(value.to_string()));
        }
        am.updated_at = Set(now);
        let updated = am.update(db).await?;
        return Ok(model_to_agent_profile(updated));
    }

    let model = agent_profiles::ActiveModel {
        id: Set(gen_id()),
        conversation_id: Set(conversation_id.to_string()),
        workspace_root: Set(Some(
            workspace_root
                .map(ToString::to_string)
                .unwrap_or_else(|| default_workspace_root(conversation_id)),
        )),
        permission_mode: Set(permission_mode.unwrap_or("default").to_string()),
        default_runner_kind: Set(default_runner_kind.unwrap_or("sdk").to_string()),
        default_provider_id: Set(default_provider_id.map(ToString::to_string)),
        default_model_id: Set(default_model_id.map(ToString::to_string)),
        created_at: Set(now.clone()),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;

    Ok(model_to_agent_profile(model))
}

pub async fn ensure_profile_from_session(
    db: &DatabaseConnection,
    session: &AgentSession,
) -> Result<AgentProfile> {
    upsert_profile(
        db,
        &session.conversation_id,
        session.cwd.as_deref(),
        Some(&session.permission_mode),
        None,
        None,
        None,
    )
    .await
}

pub async fn migrate_existing_sessions_to_profiles(db: &DatabaseConnection) -> Result<u64> {
    let sessions = agent_sessions::Entity::find().all(db).await?;
    let mut count = 0u64;
    for session in sessions {
        let existing = get_profile_by_conversation_id(db, &session.conversation_id).await?;
        if existing.is_some() {
            continue;
        }
        upsert_profile(
            db,
            &session.conversation_id,
            session.cwd.as_deref(),
            Some(&session.permission_mode),
            None,
            None,
            None,
        )
        .await?;
        count += 1;
    }
    Ok(count)
}
