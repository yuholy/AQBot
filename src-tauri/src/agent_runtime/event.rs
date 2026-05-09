use aqbot_core::repo::agent_run;
use aqbot_core::types::{AgentRunEvent, AgentRunStep};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use std::collections::HashMap;

pub struct AgentEventRecorder {
    pub run_id: String,
    tool_steps: HashMap<String, String>,
}

impl AgentEventRecorder {
    pub fn new(run_id: String) -> Self {
        Self {
            run_id,
            tool_steps: HashMap::new(),
        }
    }

    pub async fn append<T: Serialize>(
        &self,
        db: &DatabaseConnection,
        step_id: Option<&str>,
        event_type: &str,
        payload: &T,
    ) -> Result<AgentRunEvent, String> {
        let payload_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;
        agent_run::append_run_event(db, &self.run_id, step_id, event_type, &payload_json)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn create_step<T: Serialize>(
        &mut self,
        db: &DatabaseConnection,
        step_kind: &str,
        title: Option<&str>,
        input: Option<&T>,
    ) -> Result<AgentRunStep, String> {
        let input_json = match input {
            Some(value) => Some(serde_json::to_string(value).map_err(|e| e.to_string())?),
            None => None,
        };
        agent_run::create_step(
            db,
            &self.run_id,
            None,
            step_kind,
            "running",
            title,
            input_json.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn finish_step<T: Serialize>(
        &self,
        db: &DatabaseConnection,
        step_id: &str,
        status: &str,
        output: Option<&T>,
    ) -> Result<(), String> {
        let output_json = match output {
            Some(value) => Some(serde_json::to_string(value).map_err(|e| e.to_string())?),
            None => None,
        };
        agent_run::update_step(db, step_id, status, output_json.as_deref())
            .await
            .map_err(|e| e.to_string())
    }

    pub fn bind_tool_step(&mut self, tool_use_id: &str, step_id: &str) {
        self.tool_steps
            .insert(tool_use_id.to_string(), step_id.to_string());
    }

    pub fn tool_step_id(&self, tool_use_id: &str) -> Option<&str> {
        self.tool_steps.get(tool_use_id).map(String::as_str)
    }
}
