use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use crate::types::{Tool, ToolError, ToolInputSchema, ToolResult, ToolUseContext};

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const MAX_OUTPUT_SIZE: usize = 100_000;

/// Destructive command patterns that should be flagged.
const DESTRUCTIVE_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "rm -rf .",
    "git push --force",
    "git push -f",
    "git reset --hard",
    "chmod 777",
    "chmod -R 777",
    "> /dev/sda",
    "mkfs.",
    "dd if=",
    ":(){ :|:& };:",
];

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Executes a given bash command and returns its output. Use for system commands and terminal operations that require shell execution."
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: HashMap::from([
                (
                    "command".to_string(),
                    json!({
                        "type": "string",
                        "description": "The command to execute"
                    }),
                ),
                (
                    "timeout".to_string(),
                    json!({
                        "type": "number",
                        "description": "Optional timeout in milliseconds (max 600000)"
                    }),
                ),
                (
                    "description".to_string(),
                    json!({
                        "type": "string",
                        "description": "Clear description of what this command does"
                    }),
                ),
            ]),
            required: vec!["command".to_string()],
            additional_properties: Some(false),
        }
    }

    fn is_read_only(&self, input: &Value) -> bool {
        let command = input.get("command").and_then(|c| c.as_str()).unwrap_or("");

        // Check if command starts with a read-only command
        let cmd_trimmed = command.trim();
        let first_cmd = cmd_trimmed.split_whitespace().next().unwrap_or("");
        let single_word_reads = [
            "ls", "cat", "head", "tail", "find", "grep", "rg", "wc", "pwd", "echo", "which",
            "type", "file", "stat", "du", "df",
        ];
        let prefix_reads = [
            "git status",
            "git log",
            "git diff",
            "git show",
            "git branch",
            "cargo check",
            "cargo test --no-run",
            "rustc --version",
        ];

        single_word_reads.contains(&first_cmd)
            || prefix_reads.iter().any(|p| cmd_trimmed.starts_with(p))
    }

    async fn call(&self, input: Value, context: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let command = input
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'command' field".to_string()))?;

        // Security check
        if let Some(warning) = check_destructive(command) {
            return Ok(ToolResult::error(format!(
                "Potentially destructive command detected: {}. Proceed with caution.",
                warning
            )));
        }

        let timeout_ms = input
            .get("timeout")
            .and_then(|t| t.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);

        let output = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            run_command(command, &context.working_dir),
        )
        .await;

        match output {
            Ok(Ok((stdout, stderr, exit_code))) => {
                let mut result = String::new();

                if !stdout.is_empty() {
                    result.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str("STDERR:\n");
                    result.push_str(&stderr);
                }

                if result.len() > MAX_OUTPUT_SIZE {
                    result.truncate(MAX_OUTPUT_SIZE);
                    result.push_str("\n... (output truncated)");
                }

                if exit_code != 0 {
                    result.push_str(&format!("\n\nExit code: {}", exit_code));
                }

                if result.is_empty() {
                    result = "(no output)".to_string();
                }

                Ok(if exit_code != 0 {
                    ToolResult::error(result)
                } else {
                    ToolResult::text(result)
                })
            }
            Ok(Err(e)) => Ok(ToolResult::error(e)),
            Err(_) => Ok(ToolResult::error(format!(
                "Command timed out after {}ms",
                timeout_ms
            ))),
        }
    }
}

async fn run_command(
    command: &str,
    working_dir: &str,
) -> Result<(String, String, i32), String> {
    validate_workspace_command(command, working_dir)?;

    let shell_path = crate::mcp::shell_path::get_shell_path();
    let shells = build_shell_runners(command);
    let mut last_error: Option<String> = None;

    for (index, shell) in shells.iter().enumerate() {
        let mut cmd = Command::new(&shell.program);
        cmd.args(&shell.args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if !shell_path.is_empty() {
            cmd.env("PATH", &shell_path);
        }

        match cmd.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                let decorated_stdout = decorate_output(&shell.label, working_dir, stdout, false);
                let decorated_stderr = decorate_output(&shell.label, working_dir, stderr, true);
                return Ok((decorated_stdout, decorated_stderr, exit_code));
            }
            Err(error) => {
                let detail = format!(
                    "Command failed to start via {} in {}: {}",
                    shell.label, working_dir, error
                );
                if index + 1 == shells.len() {
                    last_error = Some(detail);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "Command failed before execution".to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellRunner {
    label: &'static str,
    program: String,
    args: Vec<String>,
}

fn build_shell_runners(command: &str) -> Vec<ShellRunner> {
    select_shell_runners(
        command,
        cfg!(windows),
        std::env::var("ComSpec").ok(),
        program_exists,
    )
}

fn select_shell_runners<F>(
    command: &str,
    is_windows: bool,
    comspec: Option<String>,
    mut has_program: F,
) -> Vec<ShellRunner>
where
    F: FnMut(&str) -> bool,
{
    if is_windows {
        let prefer = if command_looks_like_cmd(command) {
            "cmd"
        } else if command_looks_like_bash(command) {
            "bash"
        } else {
            "powershell"
        };

        let mut runners = Vec::new();
        for shell_name in preferred_windows_shell_order(prefer) {
            match shell_name {
                "powershell" => {
                    for candidate in ["pwsh.exe", "pwsh", "powershell.exe", "powershell"] {
                        if has_program(candidate) {
                            runners.push(ShellRunner {
                                label: "powershell",
                                program: candidate.to_string(),
                                args: vec![
                                    "-NoLogo".to_string(),
                                    "-NoProfile".to_string(),
                                    "-NonInteractive".to_string(),
                                    "-Command".to_string(),
                                    wrap_powershell_command(command),
                                ],
                            });
                            break;
                        }
                    }
                }
                "cmd" => {
                    runners.push(ShellRunner {
                        label: "cmd",
                        program: comspec
                            .clone()
                            .filter(|value| !value.is_empty())
                            .unwrap_or_else(|| "cmd.exe".to_string()),
                        args: vec![
                            "/d".to_string(),
                            "/s".to_string(),
                            "/c".to_string(),
                            wrap_cmd_command(command),
                        ],
                    });
                }
                "bash" => {
                    for candidate in ["bash.exe", "bash"] {
                        if has_program(candidate) {
                            runners.push(ShellRunner {
                                label: "bash",
                                program: candidate.to_string(),
                                args: vec!["-c".to_string(), command.to_string()],
                            });
                            break;
                        }
                    }
                }
                _ => {}
            }
        }

        if runners.is_empty() {
            runners.push(ShellRunner {
                label: "cmd",
                program: comspec
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "cmd.exe".to_string()),
                args: vec![
                    "/d".to_string(),
                    "/s".to_string(),
                    "/c".to_string(),
                    wrap_cmd_command(command),
                ],
            });
        }
        return runners;
    }

    for candidate in ["/bin/bash", "bash", "/bin/sh", "sh"] {
        if has_program(candidate) {
            return vec![ShellRunner {
                label: "sh",
                program: candidate.to_string(),
                args: vec!["-c".to_string(), command.to_string()],
            }];
        }
    }

    vec![ShellRunner {
        label: "sh",
        program: "sh".to_string(),
        args: vec!["-c".to_string(), command.to_string()],
    }]
}

fn preferred_windows_shell_order(preferred: &str) -> [&'static str; 3] {
    match preferred {
        "cmd" => ["cmd", "powershell", "bash"],
        "bash" => ["bash", "powershell", "cmd"],
        _ => ["powershell", "cmd", "bash"],
    }
}

fn command_looks_like_cmd(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    let markers = [
        "dir ",
        "copy ",
        "move ",
        "type ",
        "del ",
        "rmdir ",
        "if exist ",
        ".cmd",
        ".bat",
        "%cd%",
        "%userprofile%",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn command_looks_like_bash(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    let markers = [
        "grep ",
        "sed ",
        "awk ",
        "chmod ",
        "export ",
        "/bin/",
        "./",
        "&& ls",
        "$(pwd)",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn wrap_powershell_command(command: &str) -> String {
    format!(
        "[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $OutputEncoding = [Console]::OutputEncoding; {}",
        command
    )
}

fn wrap_cmd_command(command: &str) -> String {
    format!("chcp 65001>nul & {}", command)
}

fn decorate_output(shell: &str, working_dir: &str, content: String, is_stderr: bool) -> String {
    if content.trim().is_empty() {
        return String::new();
    }

    let prefix = if is_stderr { "stderr" } else { "stdout" };
    format!("[{} {} cwd={}]\n{}", shell, prefix, working_dir, content)
}

fn validate_workspace_command(command: &str, working_dir: &str) -> Result<(), String> {
    let cwd = Path::new(working_dir)
        .canonicalize()
        .map_err(|e| format!("Cannot resolve working directory '{}': {}", working_dir, e))?;

    for target in extract_directory_targets(command) {
        if !target_is_within_workspace(&target, &cwd) {
            return Err(format!(
                "Command blocked because it changes directory outside the workspace: {}",
                target
            ));
        }
    }

    let lower = command.to_ascii_lowercase();
    let root_patterns = [
        " rg /",
        " grep /",
        " ls /",
        " dir c:\\",
        " get-childitem c:\\",
        " select-string -path c:\\",
    ];
    if root_patterns.iter().any(|pattern| lower.contains(pattern)) {
        return Err("Command blocked because it targets a filesystem root outside the workspace."
            .to_string());
    }

    Ok(())
}

fn extract_directory_targets(command: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for segment in command.split(['\n', ';', '|']) {
        let trimmed = segment.trim();
        for prefix in ["cd ", "Set-Location ", "Push-Location "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let target = rest.trim().trim_matches('"').trim_matches('\'');
                if !target.is_empty() {
                    targets.push(target.to_string());
                }
            }
        }
    }
    targets
}

fn target_is_within_workspace(target: &str, cwd: &Path) -> bool {
    if target == "." {
        return true;
    }
    if target == "/" || target == "\\" {
        return false;
    }
    if target.starts_with('~') {
        return false;
    }

    let candidate = if Path::new(target).is_absolute() {
        PathBuf::from(target)
    } else {
        cwd.join(target)
    };

    if let Ok(canonical) = candidate.canonicalize() {
        return canonical.starts_with(cwd);
    }

    if let Some(parent) = candidate.parent().and_then(|parent| parent.canonicalize().ok()) {
        return parent.starts_with(cwd);
    }

    false
}

fn program_exists(candidate: &str) -> bool {
    if candidate.contains(std::path::MAIN_SEPARATOR)
        || candidate.contains('/')
        || candidate.contains('\\')
    {
        return Path::new(candidate).is_file();
    }

    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let full_path = dir.join(candidate);
                if full_path.is_file() {
                    return true;
                }

                #[cfg(windows)]
                {
                    let exe_path = dir.join(format!("{candidate}.exe"));
                    return exe_path.is_file();
                }

                #[cfg(not(windows))]
                {
                    false
                }
            })
        })
        .unwrap_or(false)
}

fn check_destructive(command: &str) -> Option<&'static str> {
    for pattern in DESTRUCTIVE_PATTERNS {
        if command.contains(pattern) {
            return Some(pattern);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{select_shell_runners, ShellRunner};

    #[test]
    fn select_shell_runner_prefers_powershell_on_windows() {
        let runners = select_shell_runners("Get-ChildItem", true, None, |candidate| {
            matches!(candidate, "pwsh.exe")
        });

        assert_eq!(
            runners[0],
            ShellRunner {
                label: "powershell",
                program: "pwsh.exe".to_string(),
                args: vec![
                    "-NoLogo".to_string(),
                    "-NoProfile".to_string(),
                    "-NonInteractive".to_string(),
                    "-Command".to_string(),
                    "[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $OutputEncoding = [Console]::OutputEncoding; Get-ChildItem".to_string(),
                ],
            }
        );
    }

    #[test]
    fn select_shell_runner_prefers_cmd_on_windows() {
        let runners = select_shell_runners(
            "dir",
            true,
            Some("C:\\Windows\\System32\\cmd.exe".to_string()),
            |_| false,
        );

        assert_eq!(
            runners[0],
            ShellRunner {
                label: "cmd",
                program: "C:\\Windows\\System32\\cmd.exe".to_string(),
                args: vec![
                    "/d".to_string(),
                    "/s".to_string(),
                    "/c".to_string(),
                    "chcp 65001>nul & dir".to_string(),
                ],
            }
        );
    }

    #[test]
    fn select_shell_runner_prefers_bash_on_unix() {
        let runners =
            select_shell_runners("node -v", false, None, |candidate| candidate == "/bin/bash");

        assert_eq!(
            runners[0],
            ShellRunner {
                label: "sh",
                program: "/bin/bash".to_string(),
                args: vec!["-c".to_string(), "node -v".to_string()],
            }
        );
    }
}
