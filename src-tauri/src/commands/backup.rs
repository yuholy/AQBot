use crate::AppState;
use aqbot_core::repo::backup;
use aqbot_core::repo::settings::get_settings;
use aqbot_core::types::*;
use aqbot_core::webdav;
use sea_orm::DatabaseConnection;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Default)]
struct RestoreCleanup {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}

impl RestoreCleanup {
    fn track_file<P: Into<PathBuf>>(&mut self, path: P) {
        self.files.push(path.into());
    }

    fn track_dir<P: Into<PathBuf>>(&mut self, path: P) {
        self.dirs.push(path.into());
    }
}

impl Drop for RestoreCleanup {
    fn drop(&mut self) {
        for path in &self.files {
            let _ = std::fs::remove_file(path);
        }
        for path in &self.dirs {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupManifest>, String> {
    backup::list_backups(&state.sea_db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_backup(
    state: State<'_, AppState>,
    format: String,
) -> Result<BackupManifest, String> {
    let settings = get_settings(&state.sea_db)
        .await
        .map_err(|e| e.to_string())?;
    let decoded_backup_dir = aqbot_core::path_vars::decode_path_opt(&settings.backup_dir);
    let backup_dir = backup::resolve_backup_dir(decoded_backup_dir.as_deref(), &state.app_data_dir);
    let db_path = state
        .db_path
        .strip_prefix("sqlite:")
        .unwrap_or(&state.db_path);
    backup::create_backup(
        &state.sea_db,
        &format,
        &backup_dir,
        &state.app_data_dir,
        std::path::Path::new(db_path),
    )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    backup_id: String,
) -> Result<(), String> {
    let manifest = backup::get_backup(&state.sea_db, &backup_id)
        .await
        .map_err(|e| e.to_string())?;

    let backup_path = manifest.file_path.ok_or("Backup file path not available")?;
    let db_path = state
        .db_path
        .strip_prefix("sqlite:")
        .unwrap_or(&state.db_path);

    match manifest.version.as_str() {
        "json" => Err("JSON backups cannot be restored directly".to_string()),
        "sqlite" => {
            backup::restore_sqlite_backup(&backup_path, db_path)
                .await
                .map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(format!("{}-wal", db_path));
            let _ = std::fs::remove_file(format!("{}-shm", db_path));
            app.restart();

            #[allow(unreachable_code)]
            Ok(())
        }
        _ => restore_backup_zip(
            app,
            &state.app_data_dir,
            &backup_path,
            db_path,
        )
        .await,
    }
}

#[tauri::command]
pub async fn delete_backup(state: State<'_, AppState>, backup_id: String) -> Result<(), String> {
    backup::delete_backup(&state.sea_db, &backup_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn batch_delete_backups(
    state: State<'_, AppState>,
    backup_ids: Vec<String>,
) -> Result<(), String> {
    backup::batch_delete_backups(&state.sea_db, &backup_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_backup_settings(state: State<'_, AppState>) -> Result<AutoBackupSettings, String> {
    let settings = get_settings(&state.sea_db)
        .await
        .map_err(|e| e.to_string())?;
    let decoded_backup_dir = aqbot_core::path_vars::decode_path_opt(&settings.backup_dir);
    let default_dir = backup::resolve_backup_dir(None, &state.app_data_dir);
    Ok(AutoBackupSettings {
        enabled: settings.auto_backup_enabled,
        interval_hours: settings.auto_backup_interval_hours,
        max_count: settings.auto_backup_max_count,
        backup_dir: Some(
            decoded_backup_dir.unwrap_or_else(|| default_dir.to_string_lossy().to_string()),
        ),
    })
}

#[tauri::command]
pub async fn update_backup_settings(
    state: State<'_, AppState>,
    backup_settings: AutoBackupSettings,
) -> Result<(), String> {
    let mut settings = get_settings(&state.sea_db)
        .await
        .map_err(|e| e.to_string())?;
    settings.auto_backup_enabled = backup_settings.enabled;
    settings.auto_backup_interval_hours = backup_settings.interval_hours;
    settings.auto_backup_max_count = backup_settings.max_count;
    settings.backup_dir = aqbot_core::path_vars::encode_path_opt(&backup_settings.backup_dir);

    aqbot_core::repo::settings::save_settings(&state.sea_db, &settings)
        .await
        .map_err(|e| e.to_string())?;

    // Restart scheduler with new settings
    restart_auto_backup(
        &state.auto_backup_handle,
        &state.sea_db,
        &state.app_data_dir,
        &backup_settings,
    )
    .await;

    Ok(())
}

/// Start or restart the auto-backup scheduler
async fn restart_auto_backup(
    handle: &Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    db: &DatabaseConnection,
    app_data_dir: &PathBuf,
    settings: &AutoBackupSettings,
) {
    let mut guard = handle.lock().await;

    // Stop existing scheduler
    if let Some(h) = guard.take() {
        h.abort();
    }

    if !settings.enabled || settings.interval_hours == 0 {
        return;
    }

    let db = db.clone();
    let app_dir = app_data_dir.clone();
    let interval_hours = settings.interval_hours;
    let max_count = settings.max_count;
    let interval_secs = interval_hours as u64 * 3600;

    // Calculate initial delay: catch up if overdue
    let initial_delay_secs = match backup::list_backups(&db).await {
        Ok(backups) if !backups.is_empty() => {
            let last_ts = &backups[0].created_at;
            if let Ok(last_time) =
                chrono::NaiveDateTime::parse_from_str(last_ts, "%Y-%m-%d %H:%M:%S")
            {
                let elapsed = chrono::Utc::now()
                    .naive_utc()
                    .signed_duration_since(last_time)
                    .num_seconds()
                    .max(0) as u64;
                if elapsed >= interval_secs {
                    0
                } else {
                    interval_secs - elapsed
                }
            } else {
                interval_secs
            }
        }
        _ => interval_secs,
    };

    let task = tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_secs);
        // Initial wait (may be shorter if overdue)
        tokio::time::sleep(std::time::Duration::from_secs(initial_delay_secs)).await;
        loop {
            // Read current settings to get backup_dir
            let backup_dir = match get_settings(&db).await {
                Ok(s) => {
                    let decoded = aqbot_core::path_vars::decode_path_opt(&s.backup_dir);
                    backup::resolve_backup_dir(decoded.as_deref(), &app_dir)
                }
                Err(_) => backup::resolve_backup_dir(None, &app_dir),
            };

            // Create auto backup (SQLite format for speed)
            let db_path = app_dir.join("aqbot.db");
            if let Err(e) =
                backup::create_backup(&db, "zip", &backup_dir, &app_dir, &db_path).await
            {
                tracing::warn!("Auto-backup failed: {}", e);
            } else {
                tracing::info!("Auto-backup created successfully");
                // Cleanup old backups
                if let Err(e) = backup::cleanup_old_backups(&db, max_count).await {
                    tracing::warn!("Auto-backup cleanup failed: {}", e);
                }
            }
            tokio::time::sleep(interval).await;
        }
    });

    *guard = Some(task);
}

async fn restore_backup_zip(
    app: tauri::AppHandle,
    app_data_dir: &std::path::Path,
    backup_path: &str,
    current_db_path: &str,
) -> Result<(), String> {
    let mut cleanup = RestoreCleanup::default();
    let temp_dir = app_data_dir.join("_local_restore_temp");
    let _ = std::fs::remove_dir_all(&temp_dir);
    cleanup.track_dir(&temp_dir);

    let contents = webdav::extract_backup_zip(std::path::Path::new(backup_path), &temp_dir)
        .map_err(|e| e.to_string())?;

    if let Some(expected) = contents
        .metadata
        .get("db_checksum")
        .and_then(|v| v.as_str())
    {
        let ok =
            webdav::verify_db_checksum(&contents.db_path, expected).map_err(|e| e.to_string())?;
        if !ok {
            return Err("Backup checksum verification failed; file may be corrupted".to_string());
        }
    }

    let master_key_dest = app_data_dir.join("master.key");
    let safety_key_backup = temp_dir.join("_pre_local_restore_safety.key");
    if master_key_dest.exists() {
        let _ = std::fs::copy(&master_key_dest, &safety_key_backup);
        cleanup.track_file(&safety_key_backup);
        #[cfg(unix)]
        {
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&safety_key_backup, perms);
        }
    }

    if let Some(ref key_path) = contents.master_key_path {
        std::fs::copy(key_path, &master_key_dest)
            .map_err(|e| format!("Failed to restore master.key: {}", e))?;
        #[cfg(unix)]
        {
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&master_key_dest, perms);
        }
    }

    backup::restore_sqlite_backup(contents.db_path.to_str().unwrap_or(""), current_db_path)
        .await
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(format!("{}-wal", current_db_path));
    let _ = std::fs::remove_file(format!("{}-shm", current_db_path));

    if contents.has_documents {
        let docs_source = temp_dir.join("documents");
        let docs_target = aqbot_core::storage_paths::documents_root();
        if docs_source.exists() {
            copy_directory(&docs_source, &docs_target)
                .map_err(|e| format!("Failed to restore documents: {}", e))?;
        }
    }

    if contents.has_workspace {
        let ws_source = temp_dir.join("workspace");
        let ws_target = app_data_dir.join("workspace");
        if ws_source.exists() {
            copy_directory(&ws_source, &ws_target)
                .map_err(|e| format!("Failed to restore workspace: {}", e))?;
        }
    }

    if contents.has_vector_db {
        let vector_source = temp_dir.join("aqbot_home").join("vector_db");
        let vector_target = app_data_dir.join("vector_db");
        if vector_source.exists() {
            copy_directory(&vector_source, &vector_target)
                .map_err(|e| format!("Failed to restore vector_db: {}", e))?;
        }
    }

    if contents.has_ssl {
        let ssl_source = temp_dir.join("aqbot_home").join("ssl");
        let ssl_target = app_data_dir.join("ssl");
        if ssl_source.exists() {
            copy_directory(&ssl_source, &ssl_target)
                .map_err(|e| format!("Failed to restore ssl: {}", e))?;
        }
    }

    app.restart();

    #[allow(unreachable_code)]
    Ok(())
}

fn copy_directory(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
