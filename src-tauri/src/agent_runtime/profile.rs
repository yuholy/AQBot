use aqbot_core::repo::{agent_profile, agent_run, agent_session};
use aqbot_core::types::{AgentProfile, AgentSession};
use sea_orm::DatabaseConnection;

fn default_workspace_root(conversation_id: &str) -> String {
    crate::paths::aqbot_home()
        .join("workspace")
        .join(conversation_id)
        .to_string_lossy()
        .to_string()
}

pub async fn get_or_create_profile(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<AgentProfile, String> {
    if let Some(profile) = agent_profile::get_profile_by_conversation_id(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?
    {
        return Ok(profile);
    }

    if let Some(session) = agent_session::get_agent_session_by_conversation_id(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?
    {
        return agent_profile::ensure_profile_from_session(db, &session)
            .await
            .map_err(|e| e.to_string());
    }

    agent_profile::upsert_profile(
        db,
        conversation_id,
        Some(&default_workspace_root(conversation_id)),
        Some("default"),
        Some("sdk"),
        None,
        None,
    )
    .await
    .map_err(|e| e.to_string())
}

pub async fn list_profiles(db: &DatabaseConnection) -> Result<Vec<AgentProfile>, String> {
    agent_profile::list_profiles(db)
        .await
        .map_err(|e| e.to_string())
}

pub async fn update_profile_from_legacy_inputs(
    db: &DatabaseConnection,
    conversation_id: &str,
    cwd: Option<&str>,
    permission_mode: Option<&str>,
) -> Result<AgentProfile, String> {
    let profile =
        agent_profile::upsert_profile(db, conversation_id, cwd, permission_mode, None, None, None)
            .await
            .map_err(|e| e.to_string())?;

    let _ = agent_session::upsert_agent_session(db, conversation_id, cwd, permission_mode).await;
    Ok(profile)
}

pub async fn get_compat_session(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<Option<AgentSession>, String> {
    let profile = match get_or_create_profile(db, conversation_id).await {
        Ok(profile) => profile,
        Err(_) => return Ok(None),
    };

    let latest_run = agent_run::get_latest_run_for_conversation(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let (total_tokens, total_cost_usd) =
        agent_run::aggregate_usage_for_conversation(db, conversation_id)
            .await
            .map_err(|e| e.to_string())?;

    let runtime_status = latest_run
        .as_ref()
        .map(|run| match run.status.as_str() {
            "queued" | "starting" | "running" => "running".to_string(),
            "waiting_approval" => "waiting_approval".to_string(),
            "waiting_input" => "waiting_approval".to_string(),
            "failed" => "error".to_string(),
            "completed" => "completed".to_string(),
            "cancelled" => "idle".to_string(),
            other => other.to_string(),
        })
        .unwrap_or_else(|| "idle".to_string());

    Ok(Some(AgentSession {
        id: profile.id.clone(),
        conversation_id: profile.conversation_id.clone(),
        cwd: profile.workspace_root.clone(),
        permission_mode: profile.permission_mode.clone(),
        runtime_status,
        sdk_context_json: latest_run
            .as_ref()
            .and_then(|run| run.sdk_context_json.clone()),
        sdk_context_backup_json: None,
        total_tokens: total_tokens.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        total_cost_usd,
        created_at: profile.created_at.clone(),
        updated_at: latest_run
            .as_ref()
            .map(|run| {
                run.finished_at
                    .clone()
                    .unwrap_or_else(|| run.started_at.clone())
            })
            .unwrap_or_else(|| profile.updated_at.clone()),
    }))
}
