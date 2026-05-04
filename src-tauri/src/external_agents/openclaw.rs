use aqbot_core::types::{AgentTask, ExternalAgent, ExternalAgentConnectionTestResult};
use serde_json::{json, Value};

use super::custom_http::{
    dispatch_task as dispatch_generic_http,
    fetch_task as fetch_generic_http_task,
    test_connection as test_generic_http,
    ConnectorResponse,
};

fn build_openclaw_payload(task_payload: Value) -> Value {
    json!({
        "protocol": "openclaw",
        "task": task_payload,
        "prompt": task_payload
            .get("input")
            .and_then(|input| input.get("text"))
            .cloned()
            .unwrap_or_else(|| json!("")),
        "metadata": {
            "conversationId": task_payload.get("conversationId").cloned().unwrap_or(Value::Null),
            "sourceMessageId": task_payload.get("sourceMessageId").cloned().unwrap_or(Value::Null),
            "createdAt": task_payload.get("createdAt").cloned().unwrap_or(Value::Null),
        },
        "context": task_payload.get("context").cloned().unwrap_or_else(|| json!({})),
    })
}

pub async fn dispatch_task(
    agent: &ExternalAgent,
    task: &AgentTask,
    task_payload: Value,
) -> Result<ConnectorResponse, String> {
    dispatch_generic_http(agent, task, build_openclaw_payload(task_payload)).await
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
