use serde::{Deserialize, Serialize};

/// Tool risk classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    ReadOnly,
    Write,
    Execute,
}

/// Permission mode for the agent session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    FullAccess,
}

impl PermissionMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "accept_edits" => Self::AcceptEdits,
            "full_access" => Self::FullAccess,
            _ => Self::Default,
        }
    }
}

/// What the permission system decides
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAction {
    AutoAllow,
    RequireApproval,
    HardDeny,
}

fn classify_tool_risk_lowered(name_lower: &str, input: Option<&serde_json::Value>) -> RiskLevel {
    // Execute-level tools
    if matches!(name_lower, "bash" | "shell" | "run_command" | "execute")
        || name_lower.contains("exec")
        || name_lower.contains("run")
        || name_lower.contains("bash")
        || name_lower.contains("shell")
    {
        return RiskLevel::Execute;
    }

    // Structured signals: command text implies execution even when tool name is generic.
    if let Some(command) = input
        .and_then(|value| value.get("command"))
        .and_then(|value| value.as_str())
    {
        let trimmed = command.trim();
        if !trimmed.is_empty() {
            return RiskLevel::Execute;
        }
    }

    // Write-level tools
    if matches!(
        name_lower,
        "write"
            | "edit"
            | "create"
            | "delete"
            | "rename"
            | "patch"
            | "write_file"
            | "edit_file"
            | "create_file"
            | "delete_file"
            | "move"
            | "mkdir"
            | "remove"
    ) || name_lower.contains("write")
        || name_lower.contains("edit")
        || name_lower.contains("create")
        || name_lower.contains("delete")
        || name_lower.contains("patch")
    {
        return RiskLevel::Write;
    }

    if let Some(path_fields) = input {
        if path_fields.get("content").is_some()
            || path_fields.get("new_path").is_some()
            || path_fields.get("diff").is_some()
        {
            return RiskLevel::Write;
        }
    }

    // Everything else is read-only
    RiskLevel::ReadOnly
}

/// Classify a tool's risk level based on its name.
pub fn classify_tool_risk(tool_name: &str) -> RiskLevel {
    classify_tool_risk_lowered(&tool_name.to_lowercase(), None)
}

/// Classify a tool's risk level using both its name and structured input.
pub fn classify_tool_risk_with_input(tool_name: &str, input: &serde_json::Value) -> RiskLevel {
    classify_tool_risk_lowered(&tool_name.to_lowercase(), Some(input))
}

/// Decision matrix: given permission mode, risk level, and whether the tool
/// is in the "always allowed" set, return the action to take.
pub fn decide_permission(
    mode: PermissionMode,
    risk: RiskLevel,
    is_always_allowed: bool,
) -> PermissionAction {
    // If tool was previously approved with "always allow", auto-allow
    if is_always_allowed {
        return PermissionAction::AutoAllow;
    }

    match (mode, risk) {
        // Default mode: only read is auto-allowed
        (PermissionMode::Default, RiskLevel::ReadOnly) => PermissionAction::AutoAllow,
        (PermissionMode::Default, RiskLevel::Write) => PermissionAction::RequireApproval,
        (PermissionMode::Default, RiskLevel::Execute) => PermissionAction::RequireApproval,

        // Accept edits: read + write auto-allowed
        (PermissionMode::AcceptEdits, RiskLevel::ReadOnly) => PermissionAction::AutoAllow,
        (PermissionMode::AcceptEdits, RiskLevel::Write) => PermissionAction::AutoAllow,
        (PermissionMode::AcceptEdits, RiskLevel::Execute) => PermissionAction::RequireApproval,

        // Full access: everything auto-allowed
        (PermissionMode::FullAccess, _) => PermissionAction::AutoAllow,
    }
}
