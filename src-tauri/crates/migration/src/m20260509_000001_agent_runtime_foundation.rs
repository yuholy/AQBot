use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_profiles"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("conversation_id"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("workspace_root")).string().null())
                    .col(
                        ColumnDef::new(Alias::new("permission_mode"))
                            .string()
                            .not_null()
                            .default("default"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("default_runner_kind"))
                            .string()
                            .not_null()
                            .default("sdk"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("default_provider_id"))
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("default_model_id"))
                            .string()
                            .null(),
                    )
                    .col(ColumnDef::new(Alias::new("created_at")).string().not_null())
                    .col(ColumnDef::new(Alias::new("updated_at")).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_profiles_conversation")
                    .table(Alias::new("agent_profiles"))
                    .col(Alias::new("conversation_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_runs"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("conversation_id"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("profile_id")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("runner_kind"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("provider_id")).string().null())
                    .col(ColumnDef::new(Alias::new("model_id")).string().null())
                    .col(ColumnDef::new(Alias::new("status")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("prompt_snapshot"))
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("sdk_context_json")).text().null())
                    .col(ColumnDef::new(Alias::new("workspace_root")).string().null())
                    .col(ColumnDef::new(Alias::new("started_at")).string().not_null())
                    .col(ColumnDef::new(Alias::new("finished_at")).string().null())
                    .col(ColumnDef::new(Alias::new("error_summary")).text().null())
                    .col(ColumnDef::new(Alias::new("token_usage_json")).text().null())
                    .col(
                        ColumnDef::new(Alias::new("cost_usd"))
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_runs_conversation")
                    .table(Alias::new("agent_runs"))
                    .col(Alias::new("conversation_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_runs_profile")
                    .table(Alias::new("agent_runs"))
                    .col(Alias::new("profile_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_run_steps"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("run_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("parent_step_id")).string().null())
                    .col(ColumnDef::new(Alias::new("step_kind")).string().not_null())
                    .col(ColumnDef::new(Alias::new("status")).string().not_null())
                    .col(ColumnDef::new(Alias::new("title")).string().null())
                    .col(ColumnDef::new(Alias::new("input_json")).text().null())
                    .col(ColumnDef::new(Alias::new("output_json")).text().null())
                    .col(ColumnDef::new(Alias::new("started_at")).string().not_null())
                    .col(ColumnDef::new(Alias::new("finished_at")).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_run_steps_run")
                    .table(Alias::new("agent_run_steps"))
                    .col(Alias::new("run_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("agent_run_events"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("run_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("step_id")).string().null())
                    .col(ColumnDef::new(Alias::new("event_type")).string().not_null())
                    .col(ColumnDef::new(Alias::new("payload_json")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("sequence_no"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("created_at")).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_agent_run_events_run_seq")
                    .table(Alias::new("agent_run_events"))
                    .col(Alias::new("run_id"))
                    .col(Alias::new("sequence_no"))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                INSERT INTO agent_profiles (
                    id,
                    conversation_id,
                    workspace_root,
                    permission_mode,
                    default_runner_kind,
                    default_provider_id,
                    default_model_id,
                    created_at,
                    updated_at
                )
                SELECT
                    'profile_' || id,
                    conversation_id,
                    cwd,
                    permission_mode,
                    'sdk',
                    NULL,
                    NULL,
                    created_at,
                    updated_at
                FROM agent_sessions
                WHERE conversation_id NOT IN (
                    SELECT conversation_id FROM agent_profiles
                );
                "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE agent_sessions
                SET
                    runtime_status = 'idle',
                    sdk_context_json = NULL,
                    sdk_context_backup_json = NULL;
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "agent_run_events",
            "agent_run_steps",
            "agent_runs",
            "agent_profiles",
        ] {
            manager
                .drop_table(Table::drop().table(Alias::new(table)).to_owned())
                .await?;
        }
        Ok(())
    }
}
