use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ExternalAgents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExternalAgents::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ExternalAgents::Name).string().not_null())
                    .col(ColumnDef::new(ExternalAgents::Kind).string().not_null())
                    .col(ColumnDef::new(ExternalAgents::BaseUrl).string().null())
                    .col(ColumnDef::new(ExternalAgents::AuthType).string().not_null())
                    .col(ColumnDef::new(ExternalAgents::AuthConfigJson).text().null())
                    .col(
                        ColumnDef::new(ExternalAgents::CapabilitiesJson)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ExternalAgents::Enabled).integer().not_null())
                    .col(
                        ColumnDef::new(ExternalAgents::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExternalAgents::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AgentTasks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AgentTasks::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AgentTasks::ConversationId).string().null())
                    .col(ColumnDef::new(AgentTasks::SourceMessageId).string().null())
                    .col(ColumnDef::new(AgentTasks::ExternalAgentId).string().not_null())
                    .col(ColumnDef::new(AgentTasks::ExternalTaskId).string().null())
                    .col(ColumnDef::new(AgentTasks::Kind).string().not_null())
                    .col(ColumnDef::new(AgentTasks::Status).string().not_null())
                    .col(ColumnDef::new(AgentTasks::Title).string().not_null())
                    .col(
                        ColumnDef::new(AgentTasks::RequestPayloadJson)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AgentTasks::ResultPayloadJson).text().null())
                    .col(ColumnDef::new(AgentTasks::ErrorMessage).text().null())
                    .col(
                        ColumnDef::new(AgentTasks::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentTasks::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(AgentTasks::Table, AgentTasks::ExternalAgentId)
                            .to(ExternalAgents::Table, ExternalAgents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AgentTaskEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AgentTaskEvents::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AgentTaskEvents::TaskId).string().not_null())
                    .col(ColumnDef::new(AgentTaskEvents::EventType).string().not_null())
                    .col(
                        ColumnDef::new(AgentTaskEvents::PayloadJson)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AgentTaskEvents::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(AgentTaskEvents::Table, AgentTaskEvents::TaskId)
                            .to(AgentTasks::Table, AgentTasks::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_tasks_external_agent")
                    .table(AgentTasks::Table)
                    .col(AgentTasks::ExternalAgentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_tasks_conversation")
                    .table(AgentTasks::Table)
                    .col(AgentTasks::ConversationId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_task_events_task")
                    .table(AgentTaskEvents::Table)
                    .col(AgentTaskEvents::TaskId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AgentTaskEvents::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AgentTasks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExternalAgents::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ExternalAgents {
    Table,
    Id,
    Name,
    Kind,
    BaseUrl,
    AuthType,
    AuthConfigJson,
    CapabilitiesJson,
    Enabled,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum AgentTasks {
    Table,
    Id,
    ConversationId,
    SourceMessageId,
    ExternalAgentId,
    ExternalTaskId,
    Kind,
    Status,
    Title,
    RequestPayloadJson,
    ResultPayloadJson,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum AgentTaskEvents {
    Table,
    Id,
    TaskId,
    EventType,
    PayloadJson,
    CreatedAt,
}
