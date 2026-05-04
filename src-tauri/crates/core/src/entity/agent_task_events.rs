use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_task_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub task_id: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agent_tasks::Entity",
        from = "Column::TaskId",
        to = "super::agent_tasks::Column::Id"
    )]
    AgentTask,
}

impl Related<super::agent_tasks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AgentTask.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
