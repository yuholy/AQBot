use aqbot_core::types::{AgentTask, ExternalAgent, ExternalAgentConnectionTestResult};
use serde_json::Value;

use super::custom_http::{
    dispatch_task as dispatch_custom_http, test_connection as test_custom_http, ConnectorResponse,
};
use super::nanoclaw;
use super::openclaw;

pub async fn dispatch_task(
    agent: &ExternalAgent,
    task: &AgentTask,
    task_payload: Value,
) -> Result<ConnectorResponse, String> {
    match agent.kind.as_str() {
        "nanoclaw" => nanoclaw::dispatch_task(agent, task, task_payload).await,
        "openclaw" => openclaw::dispatch_task(agent, task, task_payload).await,
        "custom_http" | "generic_http" | "" => {
            dispatch_custom_http(agent, task, task_payload).await
        }
        _ => dispatch_custom_http(agent, task, task_payload).await,
    }
}

pub async fn test_connection(
    agent: &ExternalAgent,
) -> Result<ExternalAgentConnectionTestResult, String> {
    match agent.kind.as_str() {
        "nanoclaw" => nanoclaw::test_connection(agent).await,
        "openclaw" => openclaw::test_connection(agent).await,
        "custom_http" | "generic_http" | "" => test_custom_http(agent).await,
        _ => test_custom_http(agent).await,
    }
}

pub async fn fetch_task(
    agent: &ExternalAgent,
    external_task_id: &str,
) -> Result<ConnectorResponse, String> {
    match agent.kind.as_str() {
        "nanoclaw" => nanoclaw::fetch_task(agent, external_task_id).await,
        "openclaw" => openclaw::fetch_task(agent, external_task_id).await,
        "custom_http" | "generic_http" | "" => {
            super::custom_http::fetch_task(agent, external_task_id).await
        }
        _ => super::custom_http::fetch_task(agent, external_task_id).await,
    }
}
