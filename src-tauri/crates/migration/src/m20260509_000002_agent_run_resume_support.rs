use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (column, default) in [
            ("resume_capability", "'none'"),
            ("interrupted_reason", "NULL"),
            ("resume_token_json", "NULL"),
        ] {
            if !manager
                .has_column("agent_runs", column)
                .await
                .unwrap_or(false)
            {
                let mut stmt = format!("ALTER TABLE agent_runs ADD COLUMN {column} ");
                stmt.push_str(match column {
                    "resume_capability" => "TEXT NOT NULL DEFAULT 'none'",
                    "interrupted_reason" => "TEXT NULL",
                    "resume_token_json" => "TEXT NULL",
                    _ => unreachable!(),
                });
                manager.get_connection().execute_unprepared(&stmt).await?;
            }
            let _ = default;
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE agent_runs
                SET resume_capability = CASE
                    WHEN runner_kind = 'sdk' THEN 'resumable'
                    WHEN runner_kind IN ('claude_code', 'deepseek_tui') THEN 'replay_only'
                    ELSE 'none'
                END
                WHERE resume_capability IS NULL OR resume_capability = '';
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
