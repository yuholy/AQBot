use crate::entity::{agent_run_events, agent_run_steps, agent_runs};
use crate::error::{AQBotError, Result};
use crate::types::{AgentRun, AgentRunEvent, AgentRunStep};
use crate::utils::gen_id;
use sea_orm::*;

fn now_string() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn model_to_agent_run(model: agent_runs::Model) -> AgentRun {
    AgentRun {
        id: model.id,
        conversation_id: model.conversation_id,
        profile_id: model.profile_id,
        runner_kind: model.runner_kind,
        provider_id: model.provider_id,
        model_id: model.model_id,
        status: model.status,
        prompt_snapshot: model.prompt_snapshot,
        sdk_context_json: model.sdk_context_json,
        workspace_root: model.workspace_root,
        started_at: model.started_at,
        finished_at: model.finished_at,
        error_summary: model.error_summary,
        token_usage_json: model.token_usage_json,
        cost_usd: model.cost_usd,
        resume_capability: model.resume_capability,
        interrupted_reason: model.interrupted_reason,
        resume_token_json: model.resume_token_json,
    }
}

fn model_to_agent_run_step(model: agent_run_steps::Model) -> AgentRunStep {
    AgentRunStep {
        id: model.id,
        run_id: model.run_id,
        parent_step_id: model.parent_step_id,
        step_kind: model.step_kind,
        status: model.status,
        title: model.title,
        input_json: model.input_json,
        output_json: model.output_json,
        started_at: model.started_at,
        finished_at: model.finished_at,
    }
}

fn model_to_agent_run_event(model: agent_run_events::Model) -> AgentRunEvent {
    AgentRunEvent {
        id: model.id,
        run_id: model.run_id,
        step_id: model.step_id,
        event_type: model.event_type,
        payload_json: model.payload_json,
        sequence_no: model.sequence_no,
        created_at: model.created_at,
    }
}

pub async fn create_run(
    db: &DatabaseConnection,
    conversation_id: &str,
    profile_id: &str,
    runner_kind: &str,
    provider_id: Option<&str>,
    model_id: Option<&str>,
    prompt_snapshot: &str,
    sdk_context_json: Option<&str>,
    workspace_root: Option<&str>,
) -> Result<AgentRun> {
    let now = now_string();
    let resume_capability = match runner_kind {
        "sdk" => "resumable",
        "claude_code" | "deepseek_tui" => "replay_only",
        _ => "none",
    };
    let model = agent_runs::ActiveModel {
        id: Set(gen_id()),
        conversation_id: Set(conversation_id.to_string()),
        profile_id: Set(profile_id.to_string()),
        runner_kind: Set(runner_kind.to_string()),
        provider_id: Set(provider_id.map(ToString::to_string)),
        model_id: Set(model_id.map(ToString::to_string)),
        status: Set("queued".to_string()),
        prompt_snapshot: Set(prompt_snapshot.to_string()),
        sdk_context_json: Set(sdk_context_json.map(ToString::to_string)),
        workspace_root: Set(workspace_root.map(ToString::to_string)),
        started_at: Set(now),
        finished_at: Set(None),
        error_summary: Set(None),
        token_usage_json: Set(None),
        cost_usd: Set(0.0),
        resume_capability: Set(resume_capability.to_string()),
        interrupted_reason: Set(None),
        resume_token_json: Set(None),
    }
    .insert(db)
    .await?;
    Ok(model_to_agent_run(model))
}

pub async fn get_run(db: &DatabaseConnection, run_id: &str) -> Result<Option<AgentRun>> {
    let model = agent_runs::Entity::find_by_id(run_id).one(db).await?;
    Ok(model.map(model_to_agent_run))
}

pub async fn get_latest_run_for_conversation(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<Option<AgentRun>> {
    let model = agent_runs::Entity::find()
        .filter(agent_runs::Column::ConversationId.eq(conversation_id))
        .order_by_desc(agent_runs::Column::StartedAt)
        .one(db)
        .await?;
    Ok(model.map(model_to_agent_run))
}

pub async fn list_runs_for_conversation(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<Vec<AgentRun>> {
    let rows = agent_runs::Entity::find()
        .filter(agent_runs::Column::ConversationId.eq(conversation_id))
        .order_by_desc(agent_runs::Column::StartedAt)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(model_to_agent_run).collect())
}

pub async fn aggregate_usage_for_conversation(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<(i64, f64)> {
    let rows = agent_runs::Entity::find()
        .filter(agent_runs::Column::ConversationId.eq(conversation_id))
        .all(db)
        .await?;
    let mut total_tokens = 0i64;
    let mut total_cost = 0.0f64;
    for row in rows {
        total_cost += row.cost_usd;
        if let Some(json) = row.token_usage_json {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                if let Some(total) = value.get("total_tokens").and_then(|v| v.as_i64()) {
                    total_tokens += total;
                } else {
                    total_tokens += value
                        .get("input_tokens")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                        + value
                            .get("output_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                }
            }
        }
    }
    Ok((total_tokens, total_cost))
}

pub async fn list_active_runs_for_conversation(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<Vec<AgentRun>> {
    let rows = agent_runs::Entity::find()
        .filter(agent_runs::Column::ConversationId.eq(conversation_id))
        .filter(Condition::all().add(agent_runs::Column::Status.is_in([
            "queued",
            "starting",
            "running",
            "waiting_approval",
            "waiting_input",
            "cancelling",
        ])))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(model_to_agent_run).collect())
}

pub async fn update_run_status(
    db: &DatabaseConnection,
    run_id: &str,
    status: &str,
    error_summary: Option<&str>,
) -> Result<()> {
    let model = agent_runs::Entity::find_by_id(run_id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("AgentRun {}", run_id)))?;
    let mut am: agent_runs::ActiveModel = model.into();
    am.status = Set(status.to_string());
    if matches!(status, "completed" | "failed" | "cancelled") {
        am.finished_at = Set(Some(now_string()));
    }
    if let Some(value) = error_summary {
        am.error_summary = Set(Some(value.to_string()));
    }
    am.update(db).await?;
    Ok(())
}

pub async fn update_run_resume_state(
    db: &DatabaseConnection,
    run_id: &str,
    resume_capability: Option<&str>,
    interrupted_reason: Option<Option<&str>>,
    resume_token_json: Option<Option<&str>>,
) -> Result<()> {
    let model = agent_runs::Entity::find_by_id(run_id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("AgentRun {}", run_id)))?;
    let mut am: agent_runs::ActiveModel = model.into();
    if let Some(value) = resume_capability {
        am.resume_capability = Set(value.to_string());
    }
    if let Some(value) = interrupted_reason {
        am.interrupted_reason = Set(value.map(ToString::to_string));
    }
    if let Some(value) = resume_token_json {
        am.resume_token_json = Set(value.map(ToString::to_string));
    }
    am.update(db).await?;
    Ok(())
}

pub async fn finish_run(
    db: &DatabaseConnection,
    run_id: &str,
    status: &str,
    sdk_context_json: Option<&str>,
    token_usage_json: Option<&str>,
    cost_usd: f64,
    error_summary: Option<&str>,
) -> Result<()> {
    let model = agent_runs::Entity::find_by_id(run_id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("AgentRun {}", run_id)))?;
    let mut am: agent_runs::ActiveModel = model.into();
    am.status = Set(status.to_string());
    am.finished_at = Set(Some(now_string()));
    am.sdk_context_json = Set(sdk_context_json.map(ToString::to_string));
    am.token_usage_json = Set(token_usage_json.map(ToString::to_string));
    am.cost_usd = Set(cost_usd);
    am.error_summary = Set(error_summary.map(ToString::to_string));
    am.interrupted_reason = Set(None);
    am.update(db).await?;
    Ok(())
}

pub async fn mark_incomplete_runs_interrupted(db: &DatabaseConnection) -> Result<u64> {
    let runs = agent_runs::Entity::find()
        .filter(agent_runs::Column::Status.is_in([
            "queued",
            "starting",
            "running",
            "waiting_approval",
            "waiting_input",
            "cancelling",
        ]))
        .all(db)
        .await?;

    let mut affected = 0u64;
    for run in runs {
        let mut am: agent_runs::ActiveModel = run.clone().into();
        am.status = Set("interrupted".to_string());
        am.finished_at = Set(Some(now_string()));
        am.interrupted_reason = Set(Some("app_restart".to_string()));
        am.resume_capability = Set(match run.runner_kind.as_str() {
            "sdk" => "resumable".to_string(),
            "claude_code" | "deepseek_tui" => "replay_only".to_string(),
            _ => "none".to_string(),
        });
        am.resume_token_json = Set(match run.runner_kind.as_str() {
            "sdk" => run.sdk_context_json.clone(),
            _ => None,
        });
        am.update(db).await?;

        let payload = serde_json::json!({
            "reason": "app_restart",
            "resumeCapability": match run.runner_kind.as_str() {
                "sdk" => "resumable",
                "claude_code" | "deepseek_tui" => "replay_only",
                _ => "none",
            },
        })
        .to_string();
        let _ = append_run_event(db, &run.id, None, "run_interrupted", &payload).await?;
        affected += 1;
    }
    Ok(affected)
}

pub async fn create_step(
    db: &DatabaseConnection,
    run_id: &str,
    parent_step_id: Option<&str>,
    step_kind: &str,
    status: &str,
    title: Option<&str>,
    input_json: Option<&str>,
) -> Result<AgentRunStep> {
    let now = now_string();
    let model = agent_run_steps::ActiveModel {
        id: Set(gen_id()),
        run_id: Set(run_id.to_string()),
        parent_step_id: Set(parent_step_id.map(ToString::to_string)),
        step_kind: Set(step_kind.to_string()),
        status: Set(status.to_string()),
        title: Set(title.map(ToString::to_string)),
        input_json: Set(input_json.map(ToString::to_string)),
        output_json: Set(None),
        started_at: Set(now),
        finished_at: Set(None),
    }
    .insert(db)
    .await?;
    Ok(model_to_agent_run_step(model))
}

pub async fn update_step(
    db: &DatabaseConnection,
    step_id: &str,
    status: &str,
    output_json: Option<&str>,
) -> Result<()> {
    let model = agent_run_steps::Entity::find_by_id(step_id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("AgentRunStep {}", step_id)))?;
    let mut am: agent_run_steps::ActiveModel = model.into();
    am.status = Set(status.to_string());
    am.output_json = Set(output_json.map(ToString::to_string));
    if matches!(status, "completed" | "failed" | "cancelled") {
        am.finished_at = Set(Some(now_string()));
    }
    am.update(db).await?;
    Ok(())
}

pub async fn list_run_events(db: &DatabaseConnection, run_id: &str) -> Result<Vec<AgentRunEvent>> {
    let rows = agent_run_events::Entity::find()
        .filter(agent_run_events::Column::RunId.eq(run_id))
        .order_by_asc(agent_run_events::Column::SequenceNo)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(model_to_agent_run_event).collect())
}

pub async fn append_run_event(
    db: &DatabaseConnection,
    run_id: &str,
    step_id: Option<&str>,
    event_type: &str,
    payload_json: &str,
) -> Result<AgentRunEvent> {
    let next_sequence = agent_run_events::Entity::find()
        .filter(agent_run_events::Column::RunId.eq(run_id))
        .count(db)
        .await? as i64
        + 1;
    let model = agent_run_events::ActiveModel {
        id: Set(gen_id()),
        run_id: Set(run_id.to_string()),
        step_id: Set(step_id.map(ToString::to_string)),
        event_type: Set(event_type.to_string()),
        payload_json: Set(payload_json.to_string()),
        sequence_no: Set(next_sequence),
        created_at: Set(now_string()),
    }
    .insert(db)
    .await?;
    Ok(model_to_agent_run_event(model))
}
