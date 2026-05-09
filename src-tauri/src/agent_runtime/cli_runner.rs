use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeepSeekSessionContext {
    pub deepseek_session_id: Option<String>,
    pub deepseek_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeepSeekExecJson {
    pub model: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeepSeekSavedSession {
    pub metadata: DeepSeekSavedSessionMetadata,
    pub messages: Vec<DeepSeekSavedMessage>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeepSeekSavedSessionMetadata {
    pub id: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeepSeekSavedMessage {
    pub role: String,
    #[serde(default)]
    pub content: Vec<DeepSeekSavedContentBlock>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum DeepSeekSavedContentBlock {
    Thinking { thinking: String },
    Text { text: String },
    Other(()),
}

pub fn map_claude_code_permission_mode(mode: Option<&str>) -> &'static str {
    match mode.unwrap_or("default") {
        "accept_edits" | "acceptEdits" => "acceptEdits",
        "full_access" | "bypassPermissions" => "bypassPermissions",
        "plan" => "plan",
        "auto" => "auto",
        "dont_ask" | "dontAsk" => "dontAsk",
        _ => "default",
    }
}

pub fn resolve_claude_command_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(path_os) = env::var_os("PATH") {
        for dir in env::split_paths(&path_os) {
            candidates.push(dir.join("claude.cmd"));
            candidates.push(dir.join("claude.exe"));
            candidates.push(dir.join("claude"));
            candidates.push(dir.join("claude.ps1"));
        }
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join("AppData")
                .join("Roaming")
                .join("npm")
                .join("claude.cmd"),
        );
    }
    candidates.push(PathBuf::from(r"C:\nvm4w\nodejs\claude.cmd"));
    candidates.push(PathBuf::from(r"C:\nvm4w\nodejs\claude.ps1"));

    candidates.into_iter().find(|path| path.is_file())
}

pub fn resolve_deepseek_tui_command_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(path_os) = env::var_os("PATH") {
        for dir in env::split_paths(&path_os) {
            candidates.push(dir.join("deepseek-tui.cmd"));
            candidates.push(dir.join("deepseek-tui.exe"));
            candidates.push(dir.join("deepseek-tui"));
            candidates.push(dir.join("deepseek-tui.ps1"));
        }
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join("AppData")
                .join("Roaming")
                .join("npm")
                .join("deepseek-tui.cmd"),
        );
    }
    candidates.push(PathBuf::from(r"C:\nvm4w\nodejs\deepseek-tui.cmd"));
    candidates.push(PathBuf::from(r"C:\nvm4w\nodejs\deepseek-tui.ps1"));

    candidates.into_iter().find(|path| path.is_file())
}

fn deepseek_sessions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".deepseek").join("sessions"))
}

pub fn load_deepseek_session_context(raw: Option<&str>) -> DeepSeekSessionContext {
    raw.and_then(|value| serde_json::from_str::<DeepSeekSessionContext>(value).ok())
        .unwrap_or_default()
}

pub fn serialize_deepseek_session_context(ctx: &DeepSeekSessionContext) -> Option<String> {
    serde_json::to_string(ctx).ok()
}

pub fn build_agent_content_with_thinking(text: &str, thinking: Option<&str>) -> String {
    match thinking.map(str::trim).filter(|value| !value.is_empty()) {
        Some(thinking_text) => format!(
            "<think data-aqbot=\"1\">\n{}\n</think>\n\n{}",
            thinking_text, text
        ),
        None => text.to_string(),
    }
}

fn resolve_deepseek_session_file(session_id: &str) -> Option<PathBuf> {
    let dir = deepseek_sessions_dir()?;
    let path = dir.join(format!("{}.json", session_id));
    path.is_file().then_some(path)
}

fn latest_deepseek_session_file() -> Option<PathBuf> {
    let dir = deepseek_sessions_dir()?;
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        match &latest {
            Some((current_modified, _)) if &modified <= current_modified => {}
            _ => latest = Some((modified, path)),
        }
    }
    latest.map(|(_, path)| path)
}

fn load_deepseek_saved_session(path: &Path) -> Option<DeepSeekSavedSession> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<DeepSeekSavedSession>(&raw).ok()
}

fn extract_latest_deepseek_reply(
    session: &DeepSeekSavedSession,
) -> Option<(Option<String>, Option<String>)> {
    let message = session
        .messages
        .iter()
        .rev()
        .find(|item| item.role == "assistant")?;
    let mut thinking_parts: Vec<String> = Vec::new();
    let mut text_parts: Vec<String> = Vec::new();
    for block in &message.content {
        match block {
            DeepSeekSavedContentBlock::Thinking { thinking } => {
                thinking_parts.push(thinking.clone())
            }
            DeepSeekSavedContentBlock::Text { text } => text_parts.push(text.clone()),
            DeepSeekSavedContentBlock::Other(_) => {}
        }
    }
    let thinking = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join("\n\n"))
    };
    let text = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n\n"))
    };
    Some((thinking, text))
}

pub fn build_cli_command(resolved_path: &Path) -> Command {
    #[cfg(target_os = "windows")]
    {
        let ext = resolved_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if ext == "cmd" || ext == "bat" {
            let mut command = Command::new("cmd.exe");
            command.arg("/C").arg(resolved_path);
            return command;
        }

        if ext == "ps1" {
            let mut command = Command::new("powershell.exe");
            command
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(resolved_path);
            return command;
        }
    }

    Command::new(resolved_path)
}

pub async fn run_claude_code_once(
    prompt: &str,
    cwd: Option<&str>,
    permission_mode: Option<&str>,
    model: Option<&str>,
) -> String {
    let effective_permission_mode = map_claude_code_permission_mode(permission_mode).to_string();
    if let Some(resolved_claude_path) = resolve_claude_command_path() {
        let mut command = build_cli_command(&resolved_claude_path);
        command
            .arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg("text")
            .arg("--permission-mode")
            .arg(&effective_permission_mode);
        if let Some(selected_model) = model {
            if !selected_model.trim().is_empty() {
                command.arg("--model").arg(selected_model.trim());
            }
        }

        let cwd_error = if let Some(cwd) = cwd {
            let cwd_path = Path::new(cwd);
            if cwd_path.is_dir() {
                command.current_dir(cwd_path);
                None
            } else {
                Some(format!(
                    "Claude Code failed: working directory does not exist or is not a directory.\n\n{}",
                    cwd
                ))
            }
        } else {
            None
        };

        if let Some(cwd_error) = cwd_error {
            return cwd_error;
        }

        return match timeout(Duration::from_secs(600), command.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if output.status.success() {
                    if stdout.is_empty() {
                        "Claude Code completed without text output.".to_string()
                    } else {
                        stdout
                    }
                } else {
                    let code = output
                        .status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    format!(
                        "Claude Code failed with exit code {}.\n\n{}{}{}",
                        code,
                        if stdout.is_empty() { "" } else { &stdout },
                        if !stdout.is_empty() && !stderr.is_empty() {
                            "\n\n"
                        } else {
                            ""
                        },
                        if stderr.is_empty() { "" } else { &stderr }
                    )
                }
            }
            Ok(Err(err)) => format!(
                "Claude Code failed: could not start the `claude` command.\n\n{}",
                err
            ),
            Err(_) => "Claude Code timed out after 10 minutes.".to_string(),
        };
    }

    let path_hint = env::var("PATH").unwrap_or_default();
    format!(
        "Claude Code failed: could not locate the `claude` command.\n\nPATH: {}",
        path_hint
    )
}

pub async fn run_deepseek_tui_once(
    prompt: &str,
    cwd: Option<&str>,
    auto_mode: bool,
    model: Option<&str>,
    saved_deepseek_session_id: Option<&str>,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    if let Some(resolved_deepseek_path) = resolve_deepseek_tui_command_path() {
        let mut command = build_cli_command(&resolved_deepseek_path);
        if let Some(existing_session_id) = saved_deepseek_session_id {
            if !existing_session_id.trim().is_empty() {
                command.arg("--resume").arg(existing_session_id.trim());
            }
        }
        command.arg("exec");
        if auto_mode {
            command.arg("--auto");
        }
        if let Some(selected_model) = model {
            if !selected_model.trim().is_empty() {
                command.arg("--model").arg(selected_model.trim());
            }
        }
        command.arg("--json");
        command.arg(prompt);

        let cwd_error = if let Some(cwd) = cwd {
            let cwd_path = Path::new(cwd);
            if cwd_path.is_dir() {
                command.current_dir(cwd_path);
                None
            } else {
                Some(format!(
                    "DeepSeek TUI failed: working directory does not exist or is not a directory.\n\n{}",
                    cwd
                ))
            }
        } else {
            None
        };

        if let Some(cwd_error) = cwd_error {
            return (cwd_error, None, model.map(str::to_string), None, None);
        }

        return match timeout(Duration::from_secs(600), command.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                let parsed_exec = serde_json::from_str::<DeepSeekExecJson>(&stdout).ok();
                let resolved_session_path = saved_deepseek_session_id
                    .and_then(resolve_deepseek_session_file)
                    .or_else(latest_deepseek_session_file);
                let saved_session = resolved_session_path
                    .as_ref()
                    .and_then(|path| load_deepseek_saved_session(path));
                let saved_reply = saved_session
                    .as_ref()
                    .and_then(extract_latest_deepseek_reply)
                    .unwrap_or((None, None));
                let resolved_session_id = saved_session
                    .as_ref()
                    .map(|session| session.metadata.id.clone())
                    .or_else(|| saved_deepseek_session_id.map(str::to_string));
                let resolved_model = parsed_exec
                    .as_ref()
                    .and_then(|json| json.model.clone())
                    .or_else(|| {
                        saved_session
                            .as_ref()
                            .and_then(|session| session.metadata.model.clone())
                    })
                    .or_else(|| model.map(str::to_string));

                let final_text = if output.status.success() {
                    let api_output = parsed_exec
                        .as_ref()
                        .and_then(|json| json.output.clone())
                        .filter(|value| !value.trim().is_empty());
                    let saved_output = saved_reply
                        .1
                        .clone()
                        .filter(|value| !value.trim().is_empty());
                    let combined = api_output.or(saved_output);
                    combined.unwrap_or_else(|| {
                        "DeepSeek TUI completed without text output.".to_string()
                    })
                } else {
                    let code = output
                        .status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    format!(
                        "DeepSeek TUI failed with exit code {}.\n\n{}{}{}",
                        code,
                        if stdout.is_empty() { "" } else { &stdout },
                        if !stdout.is_empty() && !stderr.is_empty() {
                            "\n\n"
                        } else {
                            ""
                        },
                        if stderr.is_empty() { "" } else { &stderr }
                    )
                };

                let context_json = resolved_session_id.as_ref().map(|session_id| {
                    serialize_deepseek_session_context(&DeepSeekSessionContext {
                        deepseek_session_id: Some(session_id.clone()),
                        deepseek_model: resolved_model.clone(),
                    })
                    .unwrap_or_default()
                });

                (
                    final_text,
                    saved_reply.0,
                    resolved_model,
                    resolved_session_id,
                    context_json,
                )
            }
            Ok(Err(err)) => (
                format!(
                    "DeepSeek TUI failed: could not start the `deepseek-tui` command.\n\n{}",
                    err
                ),
                None,
                model.map(str::to_string),
                saved_deepseek_session_id.map(str::to_string),
                None,
            ),
            Err(_) => (
                "DeepSeek TUI timed out after 10 minutes.".to_string(),
                None,
                model.map(str::to_string),
                saved_deepseek_session_id.map(str::to_string),
                None,
            ),
        };
    }

    let path_hint = env::var("PATH").unwrap_or_default();
    (
        format!(
            "DeepSeek TUI failed: could not locate the `deepseek-tui` command.\n\nPATH: {}",
            path_hint
        ),
        None,
        model.map(str::to_string),
        saved_deepseek_session_id.map(str::to_string),
        None,
    )
}
