use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE agent_profiles
                SET default_runner_kind = 'sdk'
                WHERE default_runner_kind IN ('claude_code', 'deepseek_tui');

                UPDATE agent_runs
                SET resume_capability = CASE
                    WHEN runner_kind = 'sdk' THEN 'resumable'
                    ELSE 'none'
                END,
                resume_token_json = CASE
                    WHEN runner_kind = 'sdk' THEN resume_token_json
                    ELSE NULL
                END
                WHERE runner_kind IN ('claude_code', 'deepseek_tui')
                   OR resume_capability IN ('replay_only', 'resumable', 'none');
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
