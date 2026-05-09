use aqbot_core::types::AgentRun;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRunnerKind {
    Sdk,
    ClaudeCode,
    DeepseekTui,
}

impl AgentRunnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sdk => "sdk",
            Self::ClaudeCode => "claude_code",
            Self::DeepseekTui => "deepseek_tui",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "claude_code" | "claude-code" => Self::ClaudeCode,
            "deepseek_tui" | "deepseek-tui" => Self::DeepseekTui,
            _ => Self::Sdk,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnerResumeDecision {
    pub accepted: bool,
    pub event_type: &'static str,
    pub message: String,
}

#[allow(dead_code)]
pub trait AgentRunner {
    fn kind(&self) -> AgentRunnerKind;

    fn prepare(&self, _run: &AgentRun) -> Result<(), String> {
        Ok(())
    }

    fn start(&self, _run: &AgentRun) -> Result<(), String> {
        Ok(())
    }

    fn stream_next(&self, _run: &AgentRun) -> Result<(), String> {
        Ok(())
    }

    fn request_approval(&self, _run: &AgentRun) -> Result<(), String> {
        Ok(())
    }

    fn request_user_input(&self, _run: &AgentRun) -> Result<(), String> {
        Ok(())
    }

    fn cancel(&self, _run: &AgentRun) -> Result<(), String> {
        Ok(())
    }

    fn resume(&self, run: &AgentRun) -> RunnerResumeDecision;

    fn finalize(&self, _run: &AgentRun) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SdkRunner;

#[derive(Debug, Clone, Copy)]
pub struct ClaudeCodeRunner;

#[derive(Debug, Clone, Copy)]
pub struct DeepseekTuiRunner;

impl AgentRunner for SdkRunner {
    fn kind(&self) -> AgentRunnerKind {
        AgentRunnerKind::Sdk
    }

    fn resume(&self, run: &AgentRun) -> RunnerResumeDecision {
        let has_context = run.resume_token_json.is_some() || run.sdk_context_json.is_some();
        RunnerResumeDecision {
            accepted: has_context,
            event_type: if has_context {
                "run_resumed"
            } else {
                "run_resume_rejected"
            },
            message: if has_context {
                "SDK run accepted for resume".to_string()
            } else {
                "Interrupted SDK run has no resumable context".to_string()
            },
        }
    }
}

impl AgentRunner for ClaudeCodeRunner {
    fn kind(&self) -> AgentRunnerKind {
        AgentRunnerKind::ClaudeCode
    }

    fn resume(&self, _run: &AgentRun) -> RunnerResumeDecision {
        RunnerResumeDecision {
            accepted: false,
            event_type: "run_resume_rejected",
            message: "Claude Code runs are replay-only; start a new run instead.".to_string(),
        }
    }
}

impl AgentRunner for DeepseekTuiRunner {
    fn kind(&self) -> AgentRunnerKind {
        AgentRunnerKind::DeepseekTui
    }

    fn resume(&self, _run: &AgentRun) -> RunnerResumeDecision {
        RunnerResumeDecision {
            accepted: false,
            event_type: "run_resume_rejected",
            message: "DeepSeek TUI runs are replay-only; start a new run instead.".to_string(),
        }
    }
}

pub fn runner_for_kind(kind: AgentRunnerKind) -> Box<dyn AgentRunner + Send + Sync> {
    match kind {
        AgentRunnerKind::Sdk => Box::new(SdkRunner),
        AgentRunnerKind::ClaudeCode => Box::new(ClaudeCodeRunner),
        AgentRunnerKind::DeepseekTui => Box::new(DeepseekTuiRunner),
    }
}

pub fn runner_for_run(run: &AgentRun) -> Box<dyn AgentRunner + Send + Sync> {
    runner_for_kind(AgentRunnerKind::from_str(&run.runner_kind))
}
