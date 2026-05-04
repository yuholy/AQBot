use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub conversation_id: Option<String>,
    pub source_message_id: Option<String>,
    pub external_agent_id: String,
    pub external_task_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub request_payload_json: String,
    pub result_payload_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::external_agents::Entity",
        from = "Column::ExternalAgentId",
        to = "super::external_agents::Column::Id"
    )]
    ExternalAgent,
}

impl Related<super::external_agents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ExternalAgent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
