use aqbot_core::types::{AgentTask, ExternalAgent, ExternalAgentConnectionTestResult};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthConfig {
    token: Option<String>,
    #[serde(alias = "api_key")]
    api_key: Option<String>,
    header: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorTaskPayload<'a> {
    task: Value,
    agent: &'a ExternalAgent,
}

#[derive(Debug)]
pub struct ConnectorResponse {
    pub external_task_id: Option<String>,
    pub status: String,
    pub result_payload: Value,
    pub assistant_content: Option<String>,
}

fn apply_auth(
    mut request: reqwest::RequestBuilder,
    agent: &ExternalAgent,
) -> Result<reqwest::RequestBuilder, String> {
    match agent.auth_type.as_str() {
        "bearer" => {
            if let Some(config) = auth_config(agent)? {
                if let Some(token) = config.token {
                    request = request.bearer_auth(token);
                }
            }
        }
        "api_key" => {
            if let Some(config) = auth_config(agent)? {
                if let Some(api_key) = config.api_key.or(config.token) {
                    request = request.header(
                        config.header.unwrap_or_else(|| "X-API-Key".to_string()),
                        api_key,
                    );
                }
            }
        }
        _ => {}
    }
    Ok(request)
}

pub fn parse_json_object(raw: Option<&str>, fallback: Value) -> Result<Value, String> {
    match raw {
        Some(value) if !value.trim().is_empty() => {
            serde_json::from_str(value).map_err(|e| format!("Invalid JSON: {e}"))
        }
        _ => Ok(fallback),
    }
}

fn normalize_task_status(raw: Option<&str>, has_message: bool) -> String {
    match raw.unwrap_or_default() {
        "queued" | "running" | "completed" | "failed" | "cancelled" => raw.unwrap().to_string(),
        _ if has_message => "completed".to_string(),
        _ => "running".to_string(),
    }
}

fn extract_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(|value| value.to_string())
}

fn extract_assistant_content(value: &Value) -> Option<String> {
    extract_string_field(
        value,
        &[
            "message",
            "content",
            "text",
            "answer",
            "summary",
            "finalText",
            "output",
        ],
    )
    .or_else(|| {
        value.get("result").and_then(|result| {
            extract_string_field(
                result,
                &[
                    "message",
                    "content",
                    "text",
                    "answer",
                    "summary",
                    "finalText",
                    "output",
                ],
            )
        })
    })
}

fn task_result_from_json(value: Value) -> ConnectorResponse {
    let assistant_content = extract_assistant_content(&value);
    let status = normalize_task_status(
        value.get("status").and_then(Value::as_str),
        assistant_content.is_some(),
    );
    ConnectorResponse {
        external_task_id: extract_string_field(
            &value,
            &[
                "external_task_id",
                "externalTaskId",
                "task_id",
                "taskId",
                "jobId",
                "runId",
                "sessionId",
            ],
        ),
        status,
        result_payload: value,
        assistant_content,
    }
}

fn auth_config(agent: &ExternalAgent) -> Result<Option<AuthConfig>, String> {
    match agent.auth_config_json.as_deref() {
        Some(raw) if !raw.trim().is_empty() => serde_json::from_str(raw)
            .map(Some)
            .map_err(|e| format!("Invalid auth JSON: {e}")),
        _ => Ok(None),
    }
}

pub async fn dispatch_task(
    agent: &ExternalAgent,
    task: &AgentTask,
    task_payload: Value,
) -> Result<ConnectorResponse, String> {
    if !agent.enabled {
        return Err("External agent is disabled".to_string());
    }

    let base_url = agent
        .base_url
        .as_deref()
        .ok_or_else(|| "External agent base URL is required".to_string())?
        .trim()
        .trim_end_matches('/')
        .to_string();
    if base_url.is_empty() {
        return Err("External agent base URL is required".to_string());
    }

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let request = client
        .post(format!("{base_url}/tasks"))
        .json(&ConnectorTaskPayload {
            task: task_payload,
            agent,
        });
    let request = apply_auth(request, agent)?;
    let response = request.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("External agent returned {status}: {text}"));
    }
    if text.trim().is_empty() {
        return Ok(ConnectorResponse {
            external_task_id: Some(task.id.clone()),
            status: "running".to_string(),
            result_payload: json!({ "status": "running" }),
            assistant_content: None,
        });
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => Ok(task_result_from_json(value)),
        Err(_) => Ok(ConnectorResponse {
            external_task_id: Some(task.id.clone()),
            status: "completed".to_string(),
            result_payload: json!({ "text": text }),
            assistant_content: Some(text),
        }),
    }
}

pub async fn fetch_task(
    agent: &ExternalAgent,
    external_task_id: &str,
) -> Result<ConnectorResponse, String> {
    if !agent.enabled {
        return Err("External agent is disabled".to_string());
    }

    let base_url = agent
        .base_url
        .as_deref()
        .ok_or_else(|| "External agent base URL is required".to_string())?
        .trim()
        .trim_end_matches('/')
        .to_string();
    if base_url.is_empty() {
        return Err("External agent base URL is required".to_string());
    }

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let primary = apply_auth(
        client.get(format!("{base_url}/tasks/{external_task_id}")),
        agent,
    )?;
    let response = match primary.send().await {
        Ok(response) => response,
        Err(primary_error) => {
            let fallback = apply_auth(
                client.get(format!("{base_url}/results/{external_task_id}")),
                agent,
            )?;
            fallback
                .send()
                .await
                .map_err(|_| primary_error.to_string())?
        }
    };

    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "External agent status fetch returned {status}: {text}"
        ));
    }

    if text.trim().is_empty() {
        return Ok(ConnectorResponse {
            external_task_id: Some(external_task_id.to_string()),
            status: "running".to_string(),
            result_payload: json!({ "status": "running" }),
            assistant_content: None,
        });
    }

    match serde_json::from_str::<Value>(&text) {
        Ok(value) => {
            let mut result = task_result_from_json(value);
            if result.external_task_id.is_none() {
                result.external_task_id = Some(external_task_id.to_string());
            }
            Ok(result)
        }
        Err(_) => Ok(ConnectorResponse {
            external_task_id: Some(external_task_id.to_string()),
            status: "completed".to_string(),
            result_payload: json!({ "text": text }),
            assistant_content: Some(text),
        }),
    }
}

pub async fn test_connection(
    agent: &ExternalAgent,
) -> Result<ExternalAgentConnectionTestResult, String> {
    let base_url = match agent.base_url.as_deref() {
        Some(value) if !value.trim().is_empty() => value.trim().trim_end_matches('/').to_string(),
        _ => {
            return Ok(ExternalAgentConnectionTestResult {
                ok: false,
                status: None,
                message: Some("Base URL is required".to_string()),
            });
        }
    };

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let health_request = apply_auth(client.get(format!("{base_url}/health")), agent)?;
    let response = match health_request.send().await {
        Ok(response) => Ok(response),
        Err(_) => {
            let fallback_request = apply_auth(client.get(&base_url), agent)?;
            fallback_request.send().await
        }
    };
    match response {
        Ok(response) => {
            let status = response.status();
            let message = if status.is_success() || status == StatusCode::NOT_FOUND {
                status.to_string()
            } else {
                let text = response.text().await.unwrap_or_default();
                if text.trim().is_empty() {
                    status.to_string()
                } else {
                    format!("{status}: {text}")
                }
            };
            Ok(ExternalAgentConnectionTestResult {
                ok: status.is_success() || status == StatusCode::NOT_FOUND,
                status: Some(status.as_u16()),
                message: Some(message),
            })
        }
        Err(e) => Ok(ExternalAgentConnectionTestResult {
            ok: false,
            status: None,
            message: Some(e.to_string()),
        }),
    }
}
