use crate::types::{CacheControl, SystemBlock};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

static GIT_STATUS_CACHE: LazyLock<Mutex<HashMap<String, (Instant, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const GIT_STATUS_CACHE_TTL: Duration = Duration::from_secs(10);

/// Get git status for the given working directory.
pub fn get_git_status(cwd: &str) -> String {
    let cache_key = std::fs::canonicalize(cwd)
        .unwrap_or_else(|_| std::path::PathBuf::from(cwd))
        .to_string_lossy()
        .to_string();

    if let Some(cached) = GIT_STATUS_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        if cached.0.elapsed() < GIT_STATUS_CACHE_TTL {
            return cached.1;
        }
    }

    let status = compute_git_status(cwd);
    if let Ok(mut cache) = GIT_STATUS_CACHE.lock() {
        cache.insert(cache_key, (Instant::now(), status.clone()));
    }
    status
}

fn compute_git_status(cwd: &str) -> String {
    let mut result = String::new();

    // Get branch
    if let Ok(output) = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .output()
    {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            result.push_str(&format!("Current branch: {}\n", branch));
        }
    }

    // Get status
    if let Ok(output) = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(cwd)
        .output()
    {
        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if status.is_empty() {
            result.push_str("\nWorkspace status: clean\n");
        } else {
            result.push_str("\nWorkspace status: dirty\n");
        }
    }

    result
}

/// Build system prompt blocks with context injection.
pub fn build_system_blocks(
    cwd: &str,
    custom_system_prompt: Option<&str>,
    append_system_prompt: Option<&str>,
    skills_summary: Option<&str>,
) -> Vec<SystemBlock> {
    let mut blocks = Vec::new();

    // Main system prompt with cache control
    let system_prompt = if let Some(custom) = custom_system_prompt {
        custom.to_string()
    } else {
        default_system_prompt()
    };

    blocks.push(SystemBlock {
        block_type: "text".to_string(),
        text: system_prompt,
        cache_control: Some(CacheControl::ephemeral()),
    });

    // Git status context
    let git_status = get_git_status(cwd);
    if !git_status.is_empty() {
        blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: format!("gitStatus: {}", git_status),
            cache_control: None,
        });
    }

    // User context (date, project files)
    let user_context = get_user_context(cwd);
    if !user_context.is_empty() {
        blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: user_context,
            cache_control: Some(CacheControl::ephemeral()),
        });
    }

    // Skills context
    if let Some(skills) = skills_summary {
        if !skills.is_empty() {
            blocks.push(SystemBlock {
                block_type: "text".to_string(),
                text: skills.to_string(),
                cache_control: Some(CacheControl::ephemeral()),
            });
        }
    }

    // Append system prompt
    if let Some(append) = append_system_prompt {
        blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: append.to_string(),
            cache_control: None,
        });
    }

    blocks
}

fn default_system_prompt() -> String {
    "You are a helpful AI assistant with access to tools for software engineering tasks. \
     Use the available tools to help the user accomplish their goals. \
     Be concise and direct in your responses."
        .to_string()
}

fn get_user_context(cwd: &str) -> String {
    let mut context = String::new();

    // Current date
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    context.push_str(&format!("Current date: {}\n", date));

    // Check for project context files
    for filename in &["AGENTS.md", "AGENT.md", "CLAUDE.md", ".agent/AGENT.md"] {
        let path = std::path::Path::new(cwd).join(filename);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                context.push_str(&format!(
                    "\n# Project context from {}\n{}\n",
                    filename, content
                ));
            }
        }
    }

    context
}

/// Clear the cached git status.
pub fn clear_context_cache() {
    if let Ok(mut cache) = GIT_STATUS_CACHE.lock() {
        cache.clear();
    }
}
