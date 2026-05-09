use super::profile::get_or_create_profile;
use super::runner::{runner_for_run, AgentRunnerKind, RunnerResumeDecision};
use aqbot_core::repo::agent_run;
use aqbot_core::types::{AgentProfile, AgentRun};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::Emitter;

pub async fn ensure_no_active_run(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<(), String> {
    let active = agent_run::list_active_runs_for_conversation(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    if active.is_empty() {
        Ok(())
    } else {
        Err("Agent is already running".to_string())
    }
}

pub async fn start_run(
    db: &DatabaseConnection,
    conversation_id: &str,
    runner_kind: AgentRunnerKind,
    prompt: &str,
    provider_id: Option<&str>,
    model_id: Option<&str>,
    sdk_context_json: Option<&str>,
) -> Result<(AgentProfile, AgentRun), String> {
    let profile = get_or_create_profile(db, conversation_id).await?;
    let run = agent_run::create_run(
        db,
        conversation_id,
        &profile.id,
        runner_kind.as_str(),
        provider_id.or(profile.default_provider_id.as_deref()),
        model_id.or(profile.default_model_id.as_deref()),
        prompt,
        sdk_context_json,
        profile.workspace_root.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;
    agent_run::update_run_status(db, &run.id, "starting", None)
        .await
        .map_err(|e| e.to_string())?;
    Ok((profile, run))
}

pub async fn append_runtime_event<T: Serialize>(
    db: &DatabaseConnection,
    run_id: &str,
    event_type: &str,
    payload: &T,
) -> Result<(), String> {
    let payload_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    agent_run::append_run_event(db, run_id, None, event_type, &payload_json)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn record_control_event(
    db: &DatabaseConnection,
    run: &AgentRun,
    event_type: &str,
    target_id: Option<&str>,
    value: Option<&str>,
) -> Result<(), String> {
    append_runtime_event(
        db,
        &run.id,
        event_type,
        &serde_json::json!({
            "runId": run.id,
            "conversationId": run.conversation_id,
            "targetId": target_id,
            "value": value,
        }),
    )
    .await
}

pub async fn prepare_resume(
    db: &DatabaseConnection,
    run: &AgentRun,
) -> Result<RunnerResumeDecision, String> {
    append_runtime_event(
        db,
        &run.id,
        "run_resume_requested",
        &serde_json::json!({
            "runId": run.id,
            "runnerKind": run.runner_kind,
            "resumeCapability": run.resume_capability,
        }),
    )
    .await?;

    let runner = runner_for_run(run);
    let decision = runner.resume(run);
    append_runtime_event(
        db,
        &run.id,
        decision.event_type,
        &serde_json::json!({
            "runId": run.id,
            "runnerKind": run.runner_kind,
            "resumeCapability": run.resume_capability,
            "message": decision.message,
        }),
    )
    .await?;
    Ok(decision)
}

pub async fn mark_run_cancelling(db: &DatabaseConnection, run: &AgentRun) -> Result<(), String> {
    record_control_event(db, run, "run_cancel_requested", None, None).await?;
    agent_run::update_run_status(db, &run.id, "cancelling", None)
        .await
        .map_err(|e| e.to_string())
}

pub async fn resolve_permission_request(
    db: &DatabaseConnection,
    conversation_id: &str,
    tool_use_id: &str,
    decision: &str,
) -> Result<(), String> {
    if let Some(run) = agent_run::get_latest_run_for_conversation(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?
    {
        append_runtime_event(
            db,
            &run.id,
            "permission_resolved",
            &serde_json::json!({
                "toolUseId": tool_use_id,
                "decision": decision,
            }),
        )
        .await?;
        agent_run::update_run_status(db, &run.id, "running", None)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub async fn resolve_ask_request(
    db: &DatabaseConnection,
    ask_id: &str,
    answer: &str,
) -> Result<(), String> {
    let profiles = crate::agent_runtime::profile::list_profiles(db).await?;
    for profile in profiles {
        if let Some(run) = agent_run::get_latest_run_for_conversation(db, &profile.conversation_id)
            .await
            .map_err(|e| e.to_string())?
        {
            if matches!(run.status.as_str(), "waiting_input" | "waiting_approval") {
                append_runtime_event(
                    db,
                    &run.id,
                    "ask_resolved",
                    &serde_json::json!({
                        "askId": ask_id,
                        "answer": answer,
                    }),
                )
                .await?;
                agent_run::update_run_status(db, &run.id, "running", None)
                    .await
                    .map_err(|e| e.to_string())?;
                break;
            }
        }
    }
    Ok(())
}

pub async fn finalize_run_cancelled(
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<Option<AgentRun>, String> {
    let Some(run) = agent_run::get_latest_run_for_conversation(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };

    agent_run::finish_run(
        db,
        &run.id,
        "cancelled",
        None,
        None,
        0.0,
        Some("Cancelled by user"),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(agent_run::get_run(db, &run.id)
        .await
        .map_err(|e| e.to_string())?)
}

pub async fn finish_run_cancelled(
    app: &tauri::AppHandle,
    db: &DatabaseConnection,
    conversation_id: &str,
) -> Result<(), String> {
    if let Some(run) = finalize_run_cancelled(db, conversation_id).await? {
        app.emit(
            "agent-run-event",
            serde_json::json!({
                "conversationId": conversation_id,
                "runId": run.id,
                "status": "cancelled",
                "resumeCapability": run.resume_capability,
                "interruptedReason": "manual_cancel",
                "message": "Cancelled by user",
            }),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aqbot_core::repo::agent_profile;

    async fn create_test_run(
        db: &DatabaseConnection,
        conversation_id: &str,
        status: &str,
    ) -> AgentRun {
        let profile = agent_profile::upsert_profile(
            db,
            conversation_id,
            Some("workspace/test"),
            Some("default"),
            Some("sdk"),
            None,
            None,
        )
        .await
        .unwrap();
        let run = agent_run::create_run(
            db,
            conversation_id,
            &profile.id,
            "sdk",
            Some("provider-test"),
            Some("model-test"),
            "hello",
            None,
            profile.workspace_root.as_deref(),
        )
        .await
        .unwrap();
        agent_run::update_run_status(db, &run.id, status, None)
            .await
            .unwrap();
        agent_run::get_run(db, &run.id).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn resolve_permission_request_records_event_and_resumes_run() {
        let db = aqbot_core::db::create_test_pool().await.unwrap().conn;
        let run = create_test_run(&db, "conv_perm", "waiting_approval").await;

        resolve_permission_request(&db, "conv_perm", "perm_123", "allow_once")
            .await
            .unwrap();

        let updated = agent_run::get_run(&db, &run.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "running");

        let events = agent_run::list_run_events(&db, &run.id).await.unwrap();
        let event = events.last().unwrap();
        assert_eq!(event.event_type, "permission_resolved");
        assert!(event.payload_json.contains("\"toolUseId\":\"perm_123\""));
        assert!(event.payload_json.contains("\"decision\":\"allow_once\""));
    }

    #[tokio::test]
    async fn resolve_ask_request_records_event_and_resumes_run() {
        let db = aqbot_core::db::create_test_pool().await.unwrap().conn;
        let run = create_test_run(&db, "conv_ask", "waiting_input").await;

        resolve_ask_request(&db, "ask_123", "ship_it")
            .await
            .unwrap();

        let updated = agent_run::get_run(&db, &run.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "running");

        let events = agent_run::list_run_events(&db, &run.id).await.unwrap();
        let event = events.last().unwrap();
        assert_eq!(event.event_type, "ask_resolved");
        assert!(event.payload_json.contains("\"askId\":\"ask_123\""));
        assert!(event.payload_json.contains("\"answer\":\"ship_it\""));
    }

    #[tokio::test]
    async fn mark_run_cancelling_records_event_and_updates_status() {
        let db = aqbot_core::db::create_test_pool().await.unwrap().conn;
        let run = create_test_run(&db, "conv_cancel_mark", "running").await;

        mark_run_cancelling(&db, &run).await.unwrap();

        let updated = agent_run::get_run(&db, &run.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "cancelling");

        let events = agent_run::list_run_events(&db, &run.id).await.unwrap();
        let event = events.last().unwrap();
        assert_eq!(event.event_type, "run_cancel_requested");
    }

    #[tokio::test]
    async fn finalize_run_cancelled_finishes_run() {
        let db = aqbot_core::db::create_test_pool().await.unwrap().conn;
        let run = create_test_run(&db, "conv_cancel_finish", "cancelling").await;

        let finalized = finalize_run_cancelled(&db, "conv_cancel_finish")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(finalized.id, run.id);
        assert_eq!(finalized.status, "cancelled");
        assert_eq!(
            finalized.error_summary.as_deref(),
            Some("Cancelled by user")
        );
    }
}
