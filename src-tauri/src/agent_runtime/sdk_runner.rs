use super::event::AgentEventRecorder;
use crate::agent_runtime::compat::{
    create_adapter_arc, ensure_agent_assistant_message, ensure_legacy_session_for_profile,
    get_tool_input_summary, persist_agent_partial_content, provider_type_to_registry_key,
    resolve_agent_provider_id, truncate_preview, AgentCancelTokenGuard, RunningAgentGuard,
    RUNNING_AGENTS,
};
use crate::agent_runtime::payloads::{
    AgentAskUserPayload, AgentDonePayload, AgentErrorPayload, AgentPermissionRequestPayload,
    AgentRateLimitPayload, AgentStatusPayload, AgentTextPayload, AgentThinkingPayload,
    AgentToolResultPayload, AgentToolStartPayload, AgentToolUsePayload, AgentUsagePayload,
};
use crate::AppState;
use aqbot_agent::permission::{classify_tool_risk_with_input, decide_permission, PermissionAction};
use aqbot_core::repo::{
    agent_run, agent_session, conversation, message, provider, settings, skill, tool_execution,
};
use aqbot_core::types::{MessageRole, ProviderProxyConfig};
use aqbot_providers::{resolve_base_url_for_type, ProviderRequestContext};
use open_agent_sdk::{
    Agent, AgentOptions, CanUseToolFn, ContentBlock, PermissionDecision, SDKMessage, Usage,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct StartSdkRunInput {
    pub conversation_id: String,
    pub prompt: String,
    pub provider_id: String,
    pub model_id: String,
}

pub async fn start_sdk_run(
    app: tauri::AppHandle,
    state: &AppState,
    input: StartSdkRunInput,
) -> Result<(), String> {
    let StartSdkRunInput {
        conversation_id,
        prompt,
        provider_id,
        model_id,
    } = input;

    let profile =
        crate::agent_runtime::profile::get_or_create_profile(&state.sea_db, &conversation_id)
            .await?;
    let session = ensure_legacy_session_for_profile(&state.sea_db, &profile).await?;

    crate::agent_runtime::runtime::ensure_no_active_run(&state.sea_db, &conversation_id).await?;

    {
        let running = RUNNING_AGENTS.lock().unwrap();
        if running.contains_key(&conversation_id) {
            return Err("Agent is already running".to_string());
        }
    }

    let real_provider_id = resolve_agent_provider_id(&state.sea_db, &provider_id).await?;

    agent_session::update_agent_session_status(&state.sea_db, &session.id, "running")
        .await
        .map_err(|e| e.to_string())?;

    let run = match crate::agent_runtime::runtime::start_run(
        &state.sea_db,
        &conversation_id,
        crate::agent_runtime::runner::AgentRunnerKind::Sdk,
        &prompt,
        Some(&provider_id),
        Some(&model_id),
        session.sdk_context_json.as_deref(),
    )
    .await
    {
        Ok((_, run)) => run,
        Err(err) => {
            let _ = agent_session::update_agent_session_status(&state.sea_db, &session.id, "idle")
                .await;
            return Err(err);
        }
    };

    let user_message = message::create_message(
        &state.sea_db,
        &conversation_id,
        MessageRole::User,
        &prompt,
        &[],
        None,
        0,
    )
    .await
    .map_err(|e| e.to_string())?;

    let pre_conv = conversation::get_conversation(&state.sea_db, &conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let is_first_message = pre_conv.message_count <= 1;

    conversation::increment_message_count(&state.sea_db, &conversation_id)
        .await
        .map_err(|e| e.to_string())?;

    if is_first_message {
        let fallback_title = if prompt.chars().count() > 30 {
            format!("{}...", prompt.chars().take(30).collect::<String>())
        } else {
            prompt.clone()
        };
        if let Err(e) = conversation::update_conversation_title(
            &state.sea_db,
            &conversation_id,
            &fallback_title,
        )
        .await
        {
            tracing::error!("[agent] Failed to set fallback title: {}", e);
        } else {
            let _ = app.emit(
                "conversation-title-updated",
                aqbot_core::types::ConversationTitleUpdatedEvent {
                    conversation_id: conversation_id.clone(),
                    title: fallback_title,
                },
            );
        }
    }

    let prov = provider::get_provider(&state.sea_db, &real_provider_id)
        .await
        .map_err(|e| e.to_string())?;
    let key_row = provider::get_active_key(&state.sea_db, &real_provider_id)
        .await
        .map_err(|e| e.to_string())?;
    let decrypted_key = aqbot_core::crypto::decrypt_key(&key_row.key_encrypted, &state.master_key)
        .map_err(|e| e.to_string())?;
    let model_param_overrides = provider::get_model(&state.sea_db, &real_provider_id, &model_id)
        .await
        .ok()
        .and_then(|model| model.param_overrides);

    let global_settings = settings::get_settings(&state.sea_db)
        .await
        .unwrap_or_default();
    let resolved_proxy = ProviderProxyConfig::resolve(&prov.proxy_config, &global_settings);
    let ctx = ProviderRequestContext {
        api_key: decrypted_key,
        key_id: key_row.id.clone(),
        provider_id: prov.id.clone(),
        base_url: Some(resolve_base_url_for_type(
            &prov.api_host,
            &prov.provider_type,
        )),
        api_path: prov.api_path.clone(),
        proxy_config: resolved_proxy,
        custom_headers: prov
            .custom_headers
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok()),
    };

    let title_ctx = ctx.clone();
    let adapter = create_adapter_arc(&prov.provider_type)?;
    let provider_type_str = provider_type_to_registry_key(&prov.provider_type);
    let bridge = aqbot_agent::bridge::AQBotProviderBridge::new(adapter, ctx, provider_type_str)
        .map_err(|e| e.to_string())?
        .with_model_param_overrides(model_param_overrides)
        .with_app(app.clone(), conversation_id.clone());

    let permission_mode =
        aqbot_agent::permission::PermissionMode::from_str(&session.permission_mode);
    let cwd_for_check = session.cwd.clone().unwrap_or_default();
    let cancel_token = open_agent_sdk::CancellationToken::new();
    let always_allowed_map = state.agent_always_allowed.clone();
    let conv_id_for_allowed = conversation_id.clone();
    let permission_senders = state.agent_permission_senders.clone();
    let app_for_perm = app.clone();
    let conv_id_for_perm = conversation_id.clone();
    let current_assistant_id_for_perm: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
    let assistant_id_for_task = current_assistant_id_for_perm.clone();
    let db_for_perm = state.sea_db.clone();
    let cancel_token_for_perm = cancel_token.clone();
    let run_id_for_perm = run.id.clone();

    let can_use_tool: CanUseToolFn = Arc::new(move |tool_name: &str, input: &Value| {
        let tool_name = tool_name.to_string();
        let input = input.clone();
        let cwd = cwd_for_check.clone();
        let always_allowed_map = always_allowed_map.clone();
        let conv_id = conv_id_for_perm.clone();
        let conv_id_allowed = conv_id_for_allowed.clone();
        let permission_senders = permission_senders.clone();
        let app = app_for_perm.clone();
        let assistant_id = current_assistant_id_for_perm.clone();
        let db = db_for_perm.clone();
        let cancel_token = cancel_token_for_perm.clone();
        let run_id = run_id_for_perm.clone();

        Box::pin(async move {
            if cancel_token.is_cancelled() {
                return PermissionDecision::Deny("Agent cancelled".to_string());
            }

            {
                let map = always_allowed_map.lock().await;
                if let Some(set) = map.get(&conv_id_allowed) {
                    if set.contains(&tool_name) {
                        return PermissionDecision::Allow;
                    }
                }
            }

            match crate::agent_runtime::policy::evaluate_tool_use(
                &tool_name,
                &input,
                permission_mode,
                if cwd.is_empty() {
                    None
                } else {
                    Some(cwd.as_str())
                },
                false,
            ) {
                Err(decision) => decision,
                Ok(PermissionAction::AutoAllow) => PermissionDecision::Allow,
                Ok(PermissionAction::HardDeny) => {
                    PermissionDecision::Deny("Operation not permitted".to_string())
                }
                Ok(PermissionAction::RequireApproval) => {
                    let risk = classify_tool_risk_with_input(&tool_name, &input);
                    match decide_permission(permission_mode, risk, false) {
                        PermissionAction::AutoAllow => PermissionDecision::Allow,
                        PermissionAction::RequireApproval => {
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            let perm_id = format!("perm_{}", aqbot_core::utils::gen_id());

                            permission_senders.lock().await.insert(perm_id.clone(), tx);

                            let input_str = truncate_preview(
                                &serde_json::to_string(&input).unwrap_or_default(),
                                500,
                            );
                            let exec_id = tool_execution::create_tool_execution(
                                &db,
                                &conv_id,
                                assistant_id.read().await.as_deref(),
                                "__agent_sdk__",
                                &tool_name,
                                Some(&input_str),
                                Some("pending"),
                            )
                            .await
                            .ok()
                            .map(|e| e.id);

                            let risk_str = match risk {
                                aqbot_agent::permission::RiskLevel::ReadOnly => "read_only",
                                aqbot_agent::permission::RiskLevel::Write => "write",
                                aqbot_agent::permission::RiskLevel::Execute => "execute",
                            };
                            let _ = app.emit(
                                "agent-permission-request",
                                AgentPermissionRequestPayload {
                                    conversation_id: conv_id.clone(),
                                    assistant_message_id: assistant_id
                                        .read()
                                        .await
                                        .clone()
                                        .unwrap_or_default(),
                                    tool_use_id: perm_id.clone(),
                                    tool_name: tool_name.clone(),
                                    input: input.clone(),
                                    risk_level: risk_str.to_string(),
                                },
                            );
                            let _ = agent_run::update_run_status(
                                &db,
                                &run_id,
                                "waiting_approval",
                                None,
                            )
                            .await;
                            let _ = agent_run::append_run_event(
                                &db,
                                &run_id,
                                None,
                                "permission_request",
                                &serde_json::json!({
                                    "toolUseId": perm_id,
                                    "toolName": tool_name,
                                    "input": input,
                                    "riskLevel": risk_str,
                                    "executionId": exec_id,
                                })
                                .to_string(),
                            )
                            .await;

                            let final_decision = tokio::select! {
                                result = rx => match result {
                                    Ok(decision_str) => match decision_str.as_str() {
                                        "allow_once" => PermissionDecision::Allow,
                                        "allow_always" => {
                                            always_allowed_map.lock().await
                                                .entry(conv_id_allowed.clone())
                                                .or_default()
                                                .insert(tool_name.clone());
                                            PermissionDecision::Allow
                                        }
                                        "deny" => PermissionDecision::Deny(
                                            "User denied permission".to_string(),
                                        ),
                                        other => PermissionDecision::Deny(
                                            format!("Unknown decision: {}", other),
                                        ),
                                    },
                                    Err(_) => {
                                        PermissionDecision::Deny("Permission request cancelled".to_string())
                                    }
                                },
                                _ = cancel_token.cancelled() => {
                                    permission_senders.lock().await.remove(&perm_id);
                                    PermissionDecision::Deny("Agent cancelled".to_string())
                                }
                            };

                            if let Some(eid) = &exec_id {
                                let status = match &final_decision {
                                    PermissionDecision::Allow
                                    | PermissionDecision::AllowWithModifiedInput(_) => "approved",
                                    PermissionDecision::Deny(_) => "denied",
                                };
                                let _ = tool_execution::update_tool_execution_approval_status(
                                    &db, eid, status,
                                )
                                .await;
                            }

                            final_decision
                        }
                        PermissionAction::HardDeny => {
                            PermissionDecision::Deny("Operation not permitted".to_string())
                        }
                    }
                }
            }
        })
    });

    let conv = conversation::get_conversation(&state.sea_db, &conversation_id)
        .await
        .map_err(|e| e.to_string())?;

    let home = dirs::home_dir().unwrap_or_default();
    let all_skills = open_agent_sdk::skills::load_all_global(&home);
    let disabled = skill::get_disabled_skills(&state.sea_db)
        .await
        .unwrap_or_default();
    let mut registry = open_agent_sdk::skills::SkillRegistry::new();
    for skill in all_skills {
        registry.register(skill);
    }
    registry.set_disabled(disabled);
    let skills_summary = {
        let summary = registry.generate_context_summary();
        if summary.is_empty() {
            None
        } else {
            Some(summary)
        }
    };
    let skill_registry = Arc::new(tokio::sync::RwLock::new(registry));
    let skill_tool: Arc<dyn open_agent_sdk::types::Tool> = Arc::new(
        open_agent_sdk::tools::skill_tool::SkillTool::new(skill_registry),
    );

    let ask_senders = state.agent_ask_senders.clone();
    let app_for_ask = app.clone();
    let conv_id_for_ask = conversation_id.clone();
    let assistant_id_for_ask = assistant_id_for_task.clone();
    let cancel_token_for_ask = cancel_token.clone();
    let db_for_ask = state.sea_db.clone();
    let run_id_for_ask = run.id.clone();

    let ask_fn: open_agent_sdk::tools::askuser::AskUserFn = Arc::new(
        move |request: open_agent_sdk::tools::askuser::AskUserRequest| {
            let question = request.question;
            let options = request.options;
            let ask_senders = ask_senders.clone();
            let app = app_for_ask.clone();
            let conv_id = conv_id_for_ask.clone();
            let assistant_id = assistant_id_for_ask.clone();
            let cancel_token = cancel_token_for_ask.clone();
            let db = db_for_ask.clone();
            let run_id = run_id_for_ask.clone();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let ask_id = format!("ask_{}", aqbot_core::utils::gen_id());

                ask_senders.lock().await.insert(ask_id.clone(), tx);

                let _ = app.emit(
                    "agent-ask-user",
                    AgentAskUserPayload {
                        conversation_id: conv_id.clone(),
                        assistant_message_id: assistant_id.read().await.clone().unwrap_or_default(),
                        ask_id: ask_id.clone(),
                        question: question.clone(),
                        options: options.clone(),
                    },
                );
                let _ = agent_run::update_run_status(&db, &run_id, "waiting_input", None).await;
                let _ = agent_run::append_run_event(
                    &db,
                    &run_id,
                    None,
                    "ask_user",
                    &serde_json::json!({
                        "conversationId": conv_id,
                        "assistantMessageId": assistant_id.read().await.clone().unwrap_or_default(),
                        "askId": ask_id,
                        "question": question,
                        "options": options,
                    })
                    .to_string(),
                )
                .await;

                tokio::select! {
                    result = rx => result.map_err(|_| "Ask user channel closed".to_string()),
                    _ = cancel_token.cancelled() => {
                        ask_senders.lock().await.remove(&ask_id);
                        Err("Agent cancelled".to_string())
                    }
                }
            })
        },
    );

    let agent_options = AgentOptions {
        model: Some(model_id.clone()),
        provider: Some(Arc::new(bridge)),
        cwd: session.cwd.clone(),
        system_prompt: conv.system_prompt.clone(),
        skills_summary,
        ask_fn: Some(ask_fn),
        can_use_tool: Some(can_use_tool),
        custom_tools: vec![skill_tool],
        abort_signal: Some(cancel_token.clone()),
        ..Default::default()
    };

    let mut agent = Agent::new(agent_options).await.map_err(|e| e.to_string())?;

    if let Some(ref ctx_json) = session.sdk_context_json {
        match serde_json::from_str::<Vec<open_agent_sdk::Message>>(ctx_json) {
            Ok(prev_messages) => {
                tracing::info!(
                    "[agent] Restored {} messages from previous session",
                    prev_messages.len()
                );
                agent.messages = prev_messages;
            }
            Err(e) => {
                tracing::warn!("[agent] Failed to deserialize sdk_context_json: {}", e);
            }
        }
    }

    tracing::info!(
        "[agent] Agent created for conversation {}, model {}",
        conversation_id,
        model_id
    );

    let run_guard_id = aqbot_core::utils::gen_id();
    {
        let mut running = RUNNING_AGENTS.lock().unwrap();
        running.insert(conversation_id.clone(), run_guard_id.clone());
    }
    state
        .agent_cancel_tokens
        .lock()
        .await
        .insert(conversation_id.clone(), cancel_token);

    let db = state.sea_db.clone();
    let session_id = session.id.clone();
    let conv_id = conversation_id.clone();
    let user_msg_id = user_message.id.clone();
    let assistant_created_at = user_message.created_at + 1;
    let master_key = state.master_key;
    let title_prov = prov.clone();
    let title_model_id = model_id.clone();
    let title_settings = global_settings.clone();
    let title_prompt = prompt.clone();
    let cancel_tokens = state.agent_cancel_tokens.clone();

    tokio::spawn(async move {
        let _running_guard = RunningAgentGuard {
            conversation_id: conv_id.clone(),
            run_id: run_guard_id,
        };
        let _cancel_guard = AgentCancelTokenGuard {
            conversation_id: conv_id.clone(),
            tokens: cancel_tokens,
        };
        let mut recorder = AgentEventRecorder::new(run.id.clone());
        let _ = agent_run::update_run_status(&db, &run.id, "running", None).await;
        let _ = recorder
            .append(
                &db,
                None,
                "run_started",
                &serde_json::json!({
                    "conversationId": conv_id.clone(),
                    "runnerKind": "sdk",
                    "providerId": provider_id.clone(),
                    "modelId": model_id.clone(),
                }),
            )
            .await;

        tracing::info!(
            "[agent] Background task started for conversation {}",
            conv_id
        );
        let (mut rx, handle) = agent.query(&prompt).await;

        let mut result_text = String::new();
        let mut final_usage: Option<Usage> = None;
        let mut num_turns = 0u32;
        let mut cost_usd = 0.0f64;
        let mut sdk_messages: Option<Vec<open_agent_sdk::Message>> = None;
        let mut current_assistant_msg_id: Option<String> = None;
        let mut accumulated_text = String::new();
        let mut accumulated_thinking = String::new();
        let mut in_thinking_block = false;
        let mut has_streamed_deltas = false;
        let mut got_result_or_error = false;
        let mut tool_exec_map: HashMap<String, String> = HashMap::new();

        while let Some(msg) = rx.recv().await {
            match msg {
                SDKMessage::Assistant { message: msg, .. } => {
                    let mut pending_tool_uses: Vec<(String, String, Value)> = Vec::new();

                    if !has_streamed_deltas {
                        for block in &msg.content {
                            match block {
                                ContentBlock::Thinking { thinking, .. } => {
                                    if !in_thinking_block {
                                        if !accumulated_text.is_empty() {
                                            accumulated_text.push_str("\n\n");
                                        }
                                        accumulated_text.push_str("<think data-aqbot=\"1\">\n");
                                        in_thinking_block = true;
                                    }
                                    accumulated_text.push_str(thinking);
                                    accumulated_thinking.push_str(thinking);

                                    let _ = app.emit(
                                        "agent-stream-thinking",
                                        AgentThinkingPayload {
                                            conversation_id: conv_id.clone(),
                                            assistant_message_id: current_assistant_msg_id
                                                .clone()
                                                .unwrap_or_default(),
                                            thinking: thinking.clone(),
                                        },
                                    );
                                }
                                ContentBlock::Text { text } => {
                                    if in_thinking_block {
                                        accumulated_text.push_str("\n</think>\n\n");
                                        in_thinking_block = false;
                                    }
                                    accumulated_text.push_str(text);

                                    let _ = app.emit(
                                        "agent-stream-text",
                                        AgentTextPayload {
                                            conversation_id: conv_id.clone(),
                                            assistant_message_id: current_assistant_msg_id
                                                .clone()
                                                .unwrap_or_default(),
                                            text: text.clone(),
                                        },
                                    );
                                }
                                ContentBlock::ToolUse { id, name, input } => {
                                    pending_tool_uses.push((
                                        id.clone(),
                                        name.clone(),
                                        input.clone(),
                                    ));
                                }
                                _ => {}
                            }
                        }
                    } else {
                        for block in &msg.content {
                            if let ContentBlock::ToolUse { id, name, input } = block {
                                pending_tool_uses.push((id.clone(), name.clone(), input.clone()));
                            }
                        }
                    }
                    has_streamed_deltas = false;

                    if current_assistant_msg_id.is_none() {
                        let _ = ensure_agent_assistant_message(
                            &db,
                            &app,
                            &conv_id,
                            &user_msg_id,
                            assistant_created_at,
                            &accumulated_text,
                            &mut current_assistant_msg_id,
                            &assistant_id_for_task,
                        )
                        .await;
                    } else if let Some(ref mid) = current_assistant_msg_id {
                        let _ = message::update_message_content(&db, mid, &accumulated_text).await;
                    }

                    if !pending_tool_uses.is_empty() {
                        if in_thinking_block {
                            accumulated_text.push_str("\n</think>\n\n");
                            in_thinking_block = false;
                        }

                        for (sdk_id, name, input) in &pending_tool_uses {
                            let input_str = truncate_preview(
                                &serde_json::to_string(input).unwrap_or_default(),
                                500,
                            );
                            let exec_id = if let Ok(exec) = tool_execution::create_tool_execution(
                                &db,
                                &conv_id,
                                current_assistant_msg_id.as_deref(),
                                "__agent_sdk__",
                                name,
                                Some(&input_str),
                                None,
                            )
                            .await
                            {
                                let eid = exec.id.clone();
                                tool_exec_map.insert(sdk_id.clone(), eid.clone());
                                Some(eid)
                            } else {
                                None
                            };

                            if let Ok(step) = recorder
                                .create_step(&db, "tool_call", Some(name.as_str()), Some(input))
                                .await
                            {
                                recorder.bind_tool_step(sdk_id, &step.id);
                            }
                            let _ = recorder
                                .append(
                                    &db,
                                    recorder.tool_step_id(sdk_id),
                                    "tool_use",
                                    &serde_json::json!({
                                        "toolUseId": sdk_id,
                                        "toolName": name,
                                        "input": input,
                                        "executionId": exec_id,
                                    }),
                                )
                                .await;

                            let summary = get_tool_input_summary(name, input);
                            let tag_id = exec_id.as_deref().unwrap_or(sdk_id);
                            let marker = format!(
                                "\n\n<tool-call data-aqbot=\"1\" id=\"{}\" name=\"{}\">{}</tool-call>\n\n",
                                tag_id, name, summary
                            );
                            accumulated_text.push_str(&marker);

                            let _ = app.emit(
                                "agent-stream-text",
                                AgentTextPayload {
                                    conversation_id: conv_id.clone(),
                                    assistant_message_id: current_assistant_msg_id
                                        .clone()
                                        .unwrap_or_default(),
                                    text: marker,
                                },
                            );

                            let _ = app.emit(
                                "agent-tool-use",
                                AgentToolUsePayload {
                                    conversation_id: conv_id.clone(),
                                    assistant_message_id: current_assistant_msg_id
                                        .clone()
                                        .unwrap_or_default(),
                                    tool_use_id: sdk_id.clone(),
                                    tool_name: name.clone(),
                                    input: input.clone(),
                                    execution_id: exec_id,
                                },
                            );
                        }

                        if let Some(ref mid) = current_assistant_msg_id {
                            let _ =
                                message::update_message_content(&db, mid, &accumulated_text).await;
                        }
                    }
                }
                SDKMessage::ToolStart {
                    tool_use_id,
                    tool_name,
                    input,
                } => {
                    let _ = app.emit(
                        "agent-tool-start",
                        AgentToolStartPayload {
                            conversation_id: conv_id.clone(),
                            assistant_message_id: current_assistant_msg_id
                                .clone()
                                .unwrap_or_default(),
                            tool_use_id: tool_use_id.clone(),
                            tool_name: tool_name.clone(),
                            input,
                        },
                    );

                    if let Some(exec_id) = tool_exec_map.get(&tool_use_id) {
                        let _ = tool_execution::update_tool_execution_status(
                            &db, exec_id, "running", None, None,
                        )
                        .await;
                    }
                    let _ = recorder
                        .append(
                            &db,
                            recorder.tool_step_id(&tool_use_id),
                            "tool_start",
                            &serde_json::json!({
                                "toolUseId": tool_use_id,
                                "toolName": tool_name,
                            }),
                        )
                        .await;
                }
                SDKMessage::ToolResult {
                    tool_use_id,
                    tool_name,
                    content,
                    is_error,
                } => {
                    let _ = app.emit(
                        "agent-tool-result",
                        AgentToolResultPayload {
                            conversation_id: conv_id.clone(),
                            assistant_message_id: current_assistant_msg_id
                                .clone()
                                .unwrap_or_default(),
                            tool_use_id: tool_use_id.clone(),
                            tool_name: tool_name.clone(),
                            content: content.clone(),
                            is_error,
                        },
                    );

                    if let Some(exec_id) = tool_exec_map.get(&tool_use_id) {
                        let status = if is_error { "failed" } else { "success" };
                        let output_preview = truncate_preview(&content, 500);
                        let error_msg = if is_error {
                            Some(content.as_str())
                        } else {
                            None
                        };
                        let _ = tool_execution::update_tool_execution_status(
                            &db,
                            exec_id,
                            status,
                            Some(&output_preview),
                            error_msg,
                        )
                        .await;
                    }
                    if let Some(step_id) = recorder.tool_step_id(&tool_use_id).map(str::to_string) {
                        let _ = recorder
                            .finish_step(
                                &db,
                                &step_id,
                                if is_error { "failed" } else { "completed" },
                                Some(&serde_json::json!({
                                    "content": content,
                                    "isError": is_error,
                                })),
                            )
                            .await;
                    }
                    let _ = recorder
                        .append(
                            &db,
                            recorder.tool_step_id(&tool_use_id),
                            "tool_result",
                            &serde_json::json!({
                                "toolUseId": tool_use_id,
                                "toolName": tool_name,
                                "content": content,
                                "isError": is_error,
                            }),
                        )
                        .await;
                }
                SDKMessage::PermissionRequest {
                    tool_use_id,
                    tool_name,
                    input,
                    ..
                } => {
                    let input_payload = input.clone();
                    let _ = app.emit(
                        "agent-permission-request",
                        AgentPermissionRequestPayload {
                            conversation_id: conv_id.clone(),
                            assistant_message_id: current_assistant_msg_id
                                .clone()
                                .unwrap_or_default(),
                            tool_use_id: tool_use_id.clone(),
                            tool_name: tool_name.clone(),
                            input,
                            risk_level: "execute".to_string(),
                        },
                    );

                    if let Some(exec_id) = tool_exec_map.get(&tool_use_id) {
                        let _ = tool_execution::update_tool_execution_approval_status(
                            &db, exec_id, "pending",
                        )
                        .await;
                    }
                    let _ =
                        agent_run::update_run_status(&db, &run.id, "waiting_approval", None).await;
                    let _ = recorder
                        .append(
                            &db,
                            recorder.tool_step_id(&tool_use_id),
                            "permission_request",
                            &serde_json::json!({
                                "toolUseId": tool_use_id,
                                "toolName": tool_name,
                                "input": input_payload,
                            }),
                        )
                        .await;
                }
                SDKMessage::Status {
                    message: status_msg,
                }
                | SDKMessage::Progress {
                    message: status_msg,
                } => {
                    let status_event_message = status_msg.clone();
                    let _ = app.emit(
                        "agent-status",
                        AgentStatusPayload {
                            conversation_id: conv_id.clone(),
                            message: status_msg,
                        },
                    );
                    let _ = recorder
                        .append(
                            &db,
                            None,
                            "status",
                            &serde_json::json!({ "message": status_event_message }),
                        )
                        .await;
                }
                SDKMessage::RateLimit {
                    retry_after_ms,
                    message: limit_msg,
                } => {
                    let limit_event_message = limit_msg.clone();
                    let _ = app.emit(
                        "agent-rate-limit",
                        AgentRateLimitPayload {
                            conversation_id: conv_id.clone(),
                            retry_after_ms,
                            message: limit_msg,
                        },
                    );
                    let _ = recorder
                        .append(
                            &db,
                            None,
                            "rate_limit",
                            &serde_json::json!({
                                "retryAfterMs": retry_after_ms,
                                "message": limit_event_message,
                            }),
                        )
                        .await;
                }
                SDKMessage::Result {
                    text,
                    usage,
                    num_turns: t,
                    cost_usd: c,
                    messages,
                    ..
                } => {
                    got_result_or_error = true;
                    result_text = text;
                    final_usage = Some(usage);
                    num_turns = t;
                    cost_usd = c;
                    sdk_messages = Some(messages);
                }
                SDKMessage::Error { message: err_msg } => {
                    let err_event_message = err_msg.clone();
                    if err_msg.contains("reasoning_content") && err_msg.contains("thinking mode") {
                        if let Err(clear_err) =
                            agent_session::clear_sdk_context_by_conversation_id(&db, &conv_id).await
                        {
                            tracing::warn!(
                                "[agent] Failed to clear stale sdk_context after reasoning error: {}",
                                clear_err
                            );
                        }
                    }
                    let _ = app.emit(
                        "agent-error",
                        AgentErrorPayload {
                            conversation_id: conv_id.clone(),
                            assistant_message_id: current_assistant_msg_id.clone(),
                            message: err_msg,
                        },
                    );
                    let _ = recorder
                        .append(
                            &db,
                            None,
                            "error",
                            &serde_json::json!({ "message": err_event_message }),
                        )
                        .await;
                    let _ = agent_run::finish_run(
                        &db,
                        &run.id,
                        "failed",
                        None,
                        None,
                        0.0,
                        Some("Agent emitted an error event"),
                    )
                    .await;
                    let _ =
                        agent_session::update_agent_session_status(&db, &session_id, "idle").await;
                    return;
                }
                SDKMessage::ThinkingDelta { thinking } => {
                    has_streamed_deltas = true;
                    if !in_thinking_block {
                        if !accumulated_text.is_empty() {
                            accumulated_text.push_str("\n\n");
                        }
                        accumulated_text.push_str("<think data-aqbot=\"1\">\n");
                        in_thinking_block = true;
                    }
                    accumulated_text.push_str(&thinking);
                    accumulated_thinking.push_str(&thinking);
                    let assistant_message_id = persist_agent_partial_content(
                        &db,
                        &app,
                        &conv_id,
                        &user_msg_id,
                        assistant_created_at,
                        &accumulated_text,
                        &mut current_assistant_msg_id,
                        &assistant_id_for_task,
                    )
                    .await
                    .unwrap_or_default();

                    let _ = app.emit(
                        "agent-stream-thinking",
                        AgentThinkingPayload {
                            conversation_id: conv_id.clone(),
                            assistant_message_id,
                            thinking,
                        },
                    );
                }
                SDKMessage::TextDelta { text } => {
                    has_streamed_deltas = true;
                    if in_thinking_block {
                        accumulated_text.push_str("\n</think>\n\n");
                        in_thinking_block = false;
                    }
                    accumulated_text.push_str(&text);
                    let assistant_message_id = persist_agent_partial_content(
                        &db,
                        &app,
                        &conv_id,
                        &user_msg_id,
                        assistant_created_at,
                        &accumulated_text,
                        &mut current_assistant_msg_id,
                        &assistant_id_for_task,
                    )
                    .await
                    .unwrap_or_default();

                    let _ = app.emit(
                        "agent-stream-text",
                        AgentTextPayload {
                            conversation_id: conv_id.clone(),
                            assistant_message_id,
                            text,
                        },
                    );
                }
                _ => {
                    tracing::debug!("[agent] unhandled SDKMessage: {:?}", msg);
                }
            }
        }

        match handle.await {
            Ok(()) => {}
            Err(join_err) => {
                tracing::error!("[agent] Agent inner task failed: {}", join_err);
                if !got_result_or_error {
                    let _ = app.emit(
                        "agent-error",
                        AgentErrorPayload {
                            conversation_id: conv_id.clone(),
                            assistant_message_id: current_assistant_msg_id.clone(),
                            message: "Agent task crashed unexpectedly".to_string(),
                        },
                    );
                    let _ = recorder
                        .append(
                            &db,
                            None,
                            "error",
                            &serde_json::json!({ "message": "Agent task crashed unexpectedly" }),
                        )
                        .await;
                    let _ = agent_run::finish_run(
                        &db,
                        &run.id,
                        "failed",
                        None,
                        None,
                        0.0,
                        Some("Agent task crashed unexpectedly"),
                    )
                    .await;
                    let _ =
                        agent_session::update_agent_session_status(&db, &session_id, "idle").await;
                    return;
                }
            }
        }

        if !got_result_or_error {
            let _ = app.emit(
                "agent-error",
                AgentErrorPayload {
                    conversation_id: conv_id.clone(),
                    assistant_message_id: current_assistant_msg_id.clone(),
                    message: "Agent ended unexpectedly without producing a result".to_string(),
                },
            );
            let _ = recorder
                .append(
                    &db,
                    None,
                    "error",
                    &serde_json::json!({
                        "message": "Agent ended unexpectedly without producing a result",
                    }),
                )
                .await;
            let _ = agent_run::finish_run(
                &db,
                &run.id,
                "failed",
                None,
                None,
                0.0,
                Some("Agent ended unexpectedly without producing a result"),
            )
            .await;
            let _ = agent_session::update_agent_session_status(&db, &session_id, "idle").await;
            return;
        }

        let mut final_content = accumulated_text.clone();
        if in_thinking_block {
            final_content.push_str("\n</think>\n\n");
        }
        if !result_text.is_empty() && !accumulated_text.contains(&result_text) {
            final_content.push_str(&result_text);
        }

        if !final_content.is_empty() {
            if let Some(ref mid) = current_assistant_msg_id {
                let _ = message::update_message_content(&db, mid, &final_content).await;
            } else if let Ok(assist_msg) = message::create_message(
                &db,
                &conv_id,
                MessageRole::Assistant,
                &final_content,
                &[],
                Some(&user_msg_id),
                0,
            )
            .await
            {
                current_assistant_msg_id = Some(assist_msg.id.clone());
                let _ = conversation::increment_message_count(&db, &conv_id).await;
            }
        }

        let usage_payload = final_usage.as_ref().map(|u| AgentUsagePayload {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        });
        let token_usage_json = final_usage.as_ref().map(|u| {
            serde_json::json!({
                "input_tokens": u.input_tokens,
                "output_tokens": u.output_tokens,
                "total_tokens": u.input_tokens + u.output_tokens,
            })
            .to_string()
        });

        if let (Some(ref mid), Some(ref usage)) = (&current_assistant_msg_id, &final_usage) {
            let _ = message::update_message_usage(
                &db,
                mid,
                Some(usage.input_tokens as i64),
                Some(usage.output_tokens as i64),
            )
            .await;
        }

        let _ = app.emit(
            "agent-done",
            AgentDonePayload {
                conversation_id: conv_id.clone(),
                assistant_message_id: current_assistant_msg_id.clone().unwrap_or_default(),
                text: final_content.clone(),
                thinking: Some(accumulated_thinking.clone())
                    .filter(|value| !value.trim().is_empty()),
                model: Some(model_id.clone()),
                session_id: None,
                usage: usage_payload.clone(),
                num_turns: Some(num_turns),
                cost_usd: Some(cost_usd),
            },
        );
        let _ = recorder
            .append(
                &db,
                None,
                "run_finished",
                &serde_json::json!({
                    "assistantMessageId": current_assistant_msg_id.clone(),
                    "text": final_content.clone(),
                    "thinking": accumulated_thinking.clone(),
                    "usage": usage_payload,
                    "numTurns": num_turns,
                    "costUsd": cost_usd,
                }),
            )
            .await;

        if is_first_message {
            let _ = app.emit(
                "conversation-title-generating",
                aqbot_core::types::ConversationTitleGeneratingEvent {
                    conversation_id: conv_id.clone(),
                    generating: true,
                    error: None,
                },
            );

            let ai_title = crate::commands::conversations::generate_ai_title(
                &db,
                &title_prompt,
                &result_text,
                &title_prov,
                &title_ctx,
                &title_model_id,
                &title_settings,
                &master_key,
            )
            .await;

            match ai_title {
                Ok(title) => {
                    if let Err(e) =
                        conversation::update_conversation_title(&db, &conv_id, &title).await
                    {
                        let _ = app.emit(
                            "conversation-title-generating",
                            aqbot_core::types::ConversationTitleGeneratingEvent {
                                conversation_id: conv_id.clone(),
                                generating: false,
                                error: Some(format!("Failed to save title: {}", e)),
                            },
                        );
                    } else {
                        let _ = app.emit(
                            "conversation-title-updated",
                            aqbot_core::types::ConversationTitleUpdatedEvent {
                                conversation_id: conv_id.clone(),
                                title,
                            },
                        );
                        let _ = app.emit(
                            "conversation-title-generating",
                            aqbot_core::types::ConversationTitleGeneratingEvent {
                                conversation_id: conv_id.clone(),
                                generating: false,
                                error: None,
                            },
                        );
                    }
                }
                Err(err) => {
                    let _ = app.emit(
                        "conversation-title-generating",
                        aqbot_core::types::ConversationTitleGeneratingEvent {
                            conversation_id: conv_id.clone(),
                            generating: false,
                            error: Some(err),
                        },
                    );
                }
            }
        }

        let tokens_delta = final_usage
            .as_ref()
            .map(|u| (u.input_tokens + u.output_tokens) as i32)
            .unwrap_or(0);
        let sdk_context = sdk_messages
            .as_ref()
            .and_then(|msgs| serde_json::to_string(msgs).ok());
        if let Err(e) = agent_session::update_agent_session_after_query(
            &db,
            &session_id,
            "idle",
            sdk_context.as_deref(),
            tokens_delta,
            cost_usd,
        )
        .await
        {
            tracing::error!("[agent] Failed to update session after query: {}", e);
        }
        let _ = agent_run::finish_run(
            &db,
            &run.id,
            "completed",
            sdk_context.as_deref(),
            token_usage_json.as_deref(),
            cost_usd,
            None,
        )
        .await;
    });

    Ok(())
}
