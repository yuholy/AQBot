use sea_orm::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::entity::backup_manifests;
use crate::error::{AQBotError, Result};
use crate::storage_paths;
use crate::types::BackupManifest;
use crate::utils::gen_id;
use crate::webdav;

fn model_to_manifest(m: backup_manifests::Model) -> BackupManifest {
    BackupManifest {
        id: m.id,
        version: m.version,
        created_at: m.created_at,
        encrypted: m.encrypted != 0,
        checksum: m.checksum,
        object_counts_json: m.object_counts_json,
        source_app_version: m.source_app_version,
        file_path: m
            .file_path
            .as_ref()
            .map(|p| resolve_stored_backup_path(p).to_string_lossy().to_string()),
        file_size: m.file_size,
    }
}

fn backup_storage_path(path: &Path) -> String {
    if let Some(relative) = storage_paths::documents_relative_path(path) {
        return relative;
    }

    crate::path_vars::encode_path(&path.to_string_lossy())
}

fn resolve_stored_backup_path(path: &str) -> PathBuf {
    let decoded = crate::path_vars::decode_path(path);
    let path = PathBuf::from(decoded);
    if path.is_absolute() {
        path
    } else {
        storage_paths::resolve_documents_path(&path.to_string_lossy())
    }
}

/// Get the backup directory, using the configured path or defaulting to the
/// user-visible documents backups directory.
pub fn resolve_backup_dir(backup_dir_setting: Option<&str>, _app_data_dir: &Path) -> PathBuf {
    if let Some(dir) = backup_dir_setting {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    storage_paths::documents_root().join("backups")
}

/// Ensure the backup directory exists
pub fn ensure_backup_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| AQBotError::Gateway(format!("Failed to create backup directory: {}", e)))
}

/// Create a real backup file (SQLite copy or JSON export)
pub async fn create_backup(
    db: &DatabaseConnection,
    format: &str,
    backup_dir: &Path,
    app_data_dir: &Path,
    current_db_path: &Path,
) -> Result<BackupManifest> {
    ensure_backup_dir(backup_dir)?;

    let id = gen_id();
    let filename = match format {
        "json" => {
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            format!("aqbot-backup-{}.json", timestamp)
        }
        _ => webdav::generate_backup_filename(),
    };
    let file_path = backup_dir.join(&filename);

    match format {
        "json" => {
            create_json_backup(db, &file_path).await?;
        }
        _ => {
            create_full_zip_backup(db, current_db_path, app_data_dir, &file_path).await?;
        }
    }

    let file_size = std::fs::metadata(&file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let checksum = compute_file_checksum(&file_path)?;

    // Count objects for manifest
    let object_counts = count_objects(db).await?;

    let am = backup_manifests::ActiveModel {
        id: Set(id.clone()),
        version: Set(format.to_string()),
        encrypted: Set(0),
        checksum: Set(checksum),
        object_counts_json: Set(object_counts),
        source_app_version: Set(env!("CARGO_PKG_VERSION").to_string()),
        file_path: Set(Some(backup_storage_path(&file_path))),
        file_size: Set(file_size),
        ..Default::default()
    };

    am.insert(db).await?;

    get_backup(db, &id).await
}

async fn create_full_zip_backup(
    db: &DatabaseConnection,
    _current_db_path: &Path,
    app_data_dir: &Path,
    dest_zip: &Path,
) -> Result<()> {
    let temp_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_db_path = dest_zip
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("_local_backup_temp_{}.db", temp_id));
    let _ = std::fs::remove_file(&temp_db_path);

    create_sqlite_backup(db, &temp_db_path).await?;

    let documents_dir = storage_paths::documents_root();
    let workspace_dir = app_data_dir
        .parent()
        .unwrap_or(app_data_dir)
        .join("workspace");
    let master_key_path = app_data_dir.join("master.key");
    let object_counts = count_objects(db).await?;

    let result = webdav::create_backup_zip(
        &temp_db_path,
        documents_dir.exists().then_some(documents_dir.as_path()),
        workspace_dir.exists().then_some(workspace_dir.as_path()),
        master_key_path.exists().then_some(master_key_path.as_path()),
        Some(app_data_dir),
        dest_zip,
        env!("CARGO_PKG_VERSION"),
        &object_counts,
    );

    let _ = std::fs::remove_file(&temp_db_path);
    result?;

    Ok(())
}

/// Create a SQLite backup using VACUUM INTO
async fn create_sqlite_backup(db: &DatabaseConnection, dest: &Path) -> Result<()> {
    let dest_str = dest.to_string_lossy().to_string();
    // Remove existing file if present (VACUUM INTO fails otherwise)
    if dest.exists() {
        std::fs::remove_file(dest).map_err(|e| {
            AQBotError::Gateway(format!("Failed to remove existing backup file: {}", e))
        })?;
    }
    db.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        format!("VACUUM INTO '{}'", dest_str.replace('\'', "''")),
    ))
    .await
    .map_err(|e| AQBotError::Gateway(format!("VACUUM INTO failed: {}", e)))?;
    Ok(())
}

/// Create a JSON backup by exporting all important tables
async fn create_json_backup(db: &DatabaseConnection, dest: &Path) -> Result<()> {
    use crate::entity::*;

    let conversations = conversations::Entity::find().all(db).await?;
    let messages = messages::Entity::find().all(db).await?;
    let providers = providers::Entity::find().all(db).await?;
    let provider_keys = provider_keys::Entity::find().all(db).await?;
    let models = models::Entity::find().all(db).await?;
    let settings = settings::Entity::find().all(db).await?;
    let gateway_keys = gateway_keys::Entity::find().all(db).await?;

    let data = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "tables": {
            "conversations": conversations,
            "messages": messages,
            "providers": providers,
            "provider_keys": provider_keys,
            "models": models,
            "settings": settings,
            "gateway_keys": gateway_keys,
        }
    });

    let json_str = serde_json::to_string_pretty(&data)
        .map_err(|e| AQBotError::Gateway(format!("JSON serialization failed: {}", e)))?;
    std::fs::write(dest, json_str)
        .map_err(|e| AQBotError::Gateway(format!("Failed to write backup file: {}", e)))?;
    Ok(())
}

fn compute_file_checksum(path: &Path) -> Result<String> {
    let data = std::fs::read(path)
        .map_err(|e| AQBotError::Gateway(format!("Failed to read file for checksum: {}", e)))?;
    let hash = Sha256::digest(&data);
    Ok(format!("{:x}", hash))
}

async fn count_objects(db: &DatabaseConnection) -> Result<String> {
    use crate::entity::*;

    let conv_count = conversations::Entity::find().count(db).await.unwrap_or(0);
    let msg_count = messages::Entity::find().count(db).await.unwrap_or(0);
    let provider_count = providers::Entity::find().count(db).await.unwrap_or(0);

    let counts = serde_json::json!({
        "conversations": conv_count,
        "messages": msg_count,
        "providers": provider_count,
    });
    Ok(counts.to_string())
}

pub async fn list_backups(db: &DatabaseConnection) -> Result<Vec<BackupManifest>> {
    let models = backup_manifests::Entity::find()
        .order_by_desc(backup_manifests::Column::CreatedAt)
        .all(db)
        .await?;

    Ok(models.into_iter().map(model_to_manifest).collect())
}

pub async fn get_backup(db: &DatabaseConnection, id: &str) -> Result<BackupManifest> {
    let model = backup_manifests::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AQBotError::NotFound(format!("BackupManifest {}", id)))?;

    Ok(model_to_manifest(model))
}

pub async fn delete_backup(db: &DatabaseConnection, id: &str) -> Result<()> {
    let manifest = get_backup(db, id).await?;

    // Delete the file from disk if it exists
    if let Some(ref path) = manifest.file_path {
        let p = Path::new(path);
        if p.exists() {
            std::fs::remove_file(p).ok();
        }
    }

    let result = backup_manifests::Entity::delete_by_id(id).exec(db).await?;

    if result.rows_affected == 0 {
        return Err(AQBotError::NotFound(format!("BackupManifest {}", id)));
    }
    Ok(())
}

pub async fn batch_delete_backups(db: &DatabaseConnection, ids: &[String]) -> Result<()> {
    for id in ids {
        delete_backup(db, id).await?;
    }
    Ok(())
}

/// Restore from a SQLite backup by replacing the current database file.
///
/// This must only be used when the destination database is not open. For
/// in-app restores, stage the files with `stage_restore` and restart so the
/// replacement happens before SQLite opens the database.
pub async fn restore_sqlite_backup(backup_path: &str, current_db_path: &str) -> Result<()> {
    let src = Path::new(backup_path);
    if !src.exists() {
        return Err(AQBotError::NotFound(format!(
            "Backup file not found: {}",
            backup_path
        )));
    }
    std::fs::copy(src, current_db_path)
        .map_err(|e| AQBotError::Gateway(format!("Failed to restore backup: {}", e)))?;
    Ok(())
}

pub const RESTORE_MARKER_NAME: &str = "_pending_restore";

/// Stage a database and optional master key for replacement on the next app
/// startup, before the SQLite pool is opened.
pub fn stage_restore(
    source_db: &Path,
    staging_db: &Path,
    target_db_path: &Path,
    app_data_dir: &Path,
    source_master_key: Option<&Path>,
    staging_master_key: Option<&Path>,
) -> Result<()> {
    if !source_db.exists() {
        return Err(AQBotError::NotFound(format!(
            "Backup database not found: {}",
            source_db.display()
        )));
    }

    if let Some(parent) = staging_db.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AQBotError::Gateway(format!("Failed to create restore staging dir: {}", e))
        })?;
    }

    std::fs::copy(source_db, staging_db).map_err(|e| {
        AQBotError::Gateway(format!("Failed to stage database restore: {}", e))
    })?;

    let staged_key = match (source_master_key, staging_master_key) {
        (Some(src), Some(dst)) => {
            if !src.exists() {
                return Err(AQBotError::NotFound(format!(
                    "Backup master key not found: {}",
                    src.display()
                )));
            }
            std::fs::copy(src, dst).map_err(|e| {
                AQBotError::Gateway(format!("Failed to stage master.key restore: {}", e))
            })?;
            Some(dst.to_path_buf())
        }
        _ => None,
    };

    let marker_path = app_data_dir.join(RESTORE_MARKER_NAME);
    let marker_tmp = app_data_dir.join(format!("{}.tmp", RESTORE_MARKER_NAME));
    let mut marker = format!("db={}\n", staging_db.display());
    if let Some(path) = staged_key {
        marker.push_str(&format!("master_key={}\n", path.display()));
    }
    std::fs::write(&marker_tmp, marker)
        .map_err(|e| AQBotError::Gateway(format!("Failed to write restore marker: {}", e)))?;
    std::fs::rename(&marker_tmp, &marker_path)
        .map_err(|e| AQBotError::Gateway(format!("Failed to activate restore marker: {}", e)))?;

    let target_str = target_db_path.to_string_lossy();
    let _ = std::fs::remove_file(format!("{}-wal", target_str));
    let _ = std::fs::remove_file(format!("{}-shm", target_str));

    Ok(())
}

/// Apply a staged restore. This is called on startup before the database and
/// master key are loaded.
pub fn apply_staged_restore(app_data_dir: &Path, db_file_path: &Path, master_key_path: &Path) {
    let marker_path = app_data_dir.join(RESTORE_MARKER_NAME);
    if !marker_path.exists() {
        return;
    }

    let marker = match std::fs::read_to_string(&marker_path) {
        Ok(value) => value,
        Err(e) => {
            tracing::error!("Failed to read restore marker: {}", e);
            let _ = std::fs::remove_file(&marker_path);
            return;
        }
    };

    let staged_db = marker
        .lines()
        .find_map(|line| line.strip_prefix("db="))
        .map(PathBuf::from);
    let staged_key = marker
        .lines()
        .find_map(|line| line.strip_prefix("master_key="))
        .map(PathBuf::from);

    let Some(staged_db) = staged_db else {
        tracing::error!("Restore marker is missing staged database path");
        let _ = std::fs::remove_file(&marker_path);
        return;
    };

    let target_str = db_file_path.to_string_lossy();
    let _ = std::fs::remove_file(format!("{}-wal", target_str));
    let _ = std::fs::remove_file(format!("{}-shm", target_str));

    if let Some(staged_key) = staged_key.as_ref() {
        if staged_key.exists() {
            if let Err(e) = std::fs::copy(staged_key, master_key_path) {
                tracing::error!("Failed to apply staged master.key restore: {}", e);
                return;
            }
        } else {
            tracing::warn!("Staged master.key file is missing: {}", staged_key.display());
        }
    }

    if staged_db.exists() {
        if let Err(e) = std::fs::copy(&staged_db, db_file_path) {
            tracing::error!("Failed to apply staged database restore: {}", e);
            return;
        }
        tracing::info!("Staged restore applied successfully");
    } else {
        tracing::warn!("Staged database file is missing: {}", staged_db.display());
    }

    let _ = std::fs::remove_file(&staged_db);
    if let Some(staged_key) = staged_key {
        let _ = std::fs::remove_file(staged_key);
    }
    let _ = std::fs::remove_file(&marker_path);
}

/// Clean up old backups exceeding max_count (keeps most recent)
pub async fn cleanup_old_backups(db: &DatabaseConnection, max_count: u32) -> Result<u32> {
    let all = list_backups(db).await?;
    if all.len() <= max_count as usize {
        return Ok(0);
    }

    let to_delete = &all[max_count as usize..];
    let mut deleted = 0u32;
    for backup in to_delete {
        delete_backup(db, &backup.id).await?;
        deleted += 1;
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::resolve_backup_dir;
    use std::path::PathBuf;

    #[test]
    fn resolve_backup_dir_defaults_to_aqbot_backups_subdir() {
        let aqbot_home = PathBuf::from("/Users/test/.aqbot");
        let expected = crate::storage_paths::default_documents_root().join("backups");

        assert_eq!(
            resolve_backup_dir(None, &aqbot_home),
            expected
        );
        assert_eq!(
            resolve_backup_dir(Some(""), &aqbot_home),
            expected
        );
    }

    #[test]
    fn resolve_backup_dir_honors_explicit_absolute_override() {
        let aqbot_home = PathBuf::from("/Users/test/.aqbot");
        let override_dir = PathBuf::from("/Volumes/external/aqbot-backups");

        assert_eq!(
            resolve_backup_dir(Some(override_dir.to_str().unwrap()), &aqbot_home),
            override_dir
        );
    }
}
