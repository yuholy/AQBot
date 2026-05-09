use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_runs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub conversation_id: String,
    pub profile_id: String,
    pub runner_kind: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub status: String,
    pub prompt_snapshot: String,
    pub sdk_context_json: Option<String>,
    pub workspace_root: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error_summary: Option<String>,
    pub token_usage_json: Option<String>,
    pub cost_usd: f64,
    pub resume_capability: String,
    pub interrupted_reason: Option<String>,
    pub resume_token_json: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
