use aqbot_agent::permission::{classify_tool_risk_with_input, decide_permission, PermissionAction};
use aqbot_agent::security::check_path_safety;
use open_agent_sdk::PermissionDecision;
use serde_json::Value;

pub fn evaluate_tool_use(
    tool_name: &str,
    input: &Value,
    permission_mode: aqbot_agent::permission::PermissionMode,
    workspace_root: Option<&str>,
    is_always_allowed: bool,
) -> Result<PermissionAction, PermissionDecision> {
    if permission_mode != aqbot_agent::permission::PermissionMode::FullAccess {
        if let Some(workspace_root) = workspace_root.filter(|value| !value.is_empty()) {
            if let Some(deny) = check_path_safety(tool_name, input, workspace_root) {
                return Err(deny);
            }
        }
    }

    let risk = classify_tool_risk_with_input(tool_name, input);
    Ok(decide_permission(permission_mode, risk, is_always_allowed))
}
