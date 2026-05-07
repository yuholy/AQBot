use aqbot_core::types::{AgentTask, ExternalAgent, ExternalAgentConnectionTestResult};
use serde_json::{json, Value};

use super::custom_http::{
    dispatch_task as dispatch_generic_http, fetch_task as fetch_generic_http_task,
    test_connection as test_generic_http, ConnectorResponse,
};

fn build_nanoclaw_payload(task_payload: Value) -> Value {
    json!({
        "protocol": "nanoclaw",
        "task": task_payload,
        "input": task_payload.get("input").cloned().unwrap_or_else(|| json!({})),
        "context": task_payload.get("context").cloned().unwrap_or_else(|| json!({})),
        "title": task_payload.get("title").cloned().unwrap_or_else(|| json!("")),
    })
}

pub async fn dispatch_task(
    agent: &ExternalAgent,
    task: &AgentTask,
    task_payload: Value,
) -> Result<ConnectorResponse, String> {
    dispatch_generic_http(agent, task, build_nanoclaw_payload(task_payload)).await
}

pub async fn test_connection(
    agent: &ExternalAgent,
) -> Result<ExternalAgentConnectionTestResult, String> {
    test_generic_http(agent).await
}

pub async fn fetch_task(
    agent: &ExternalAgent,
    external_task_id: &str,
) -> Result<ConnectorResponse, String> {
    fetch_generic_http_task(agent, external_task_id).await
}
