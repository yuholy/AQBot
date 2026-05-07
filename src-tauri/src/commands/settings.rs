use crate::AppState;
use aqbot_core::types::*;
use tauri::AppHandle;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfilePayload {
    pub name: String,
    pub avatar_type: String,
    pub avatar_value: String,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let mut settings = aqbot_core::repo::settings::get_settings(&state.sea_db)
        .await
        .map_err(|e| e.to_string())?;
    settings.backup_dir = aqbot_core::path_vars::decode_path_opt(&settings.backup_dir);
    settings.gateway_ssl_cert_path =
        aqbot_core::path_vars::decode_path_opt(&settings.gateway_ssl_cert_path);
    settings.gateway_ssl_key_path =
        aqbot_core::path_vars::decode_path_opt(&settings.gateway_ssl_key_path);
    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: AppSettings,
) -> Result<(), String> {
    settings.backup_dir = aqbot_core::path_vars::encode_path_opt(&settings.backup_dir);
    settings.gateway_ssl_cert_path =
        aqbot_core::path_vars::encode_path_opt(&settings.gateway_ssl_cert_path);
    settings.gateway_ssl_key_path =
        aqbot_core::path_vars::encode_path_opt(&settings.gateway_ssl_key_path);
    aqbot_core::repo::settings::save_settings(&state.sea_db, &settings)
        .await
        .map_err(|e| e.to_string())?;

    crate::tray::sync_tray_language(&app, &settings.language).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_profile(state: State<'_, AppState>) -> Result<UserProfilePayload, String> {
    let settings = aqbot_core::repo::settings::get_settings(&state.sea_db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(UserProfilePayload {
        name: settings.user_profile_name,
        avatar_type: settings.user_profile_avatar_type,
        avatar_value: settings.user_profile_avatar_value,
    })
}

#[tauri::command]
pub async fn update_user_profile(
    state: State<'_, AppState>,
    profile: UserProfilePayload,
) -> Result<(), String> {
    let mut settings = aqbot_core::repo::settings::get_settings(&state.sea_db)
        .await
        .map_err(|e| e.to_string())?;

    settings.user_profile_name = profile.name;
    settings.user_profile_avatar_type = profile.avatar_type;
    settings.user_profile_avatar_value = profile.avatar_value;

    aqbot_core::repo::settings::save_settings(&state.sea_db, &settings)
        .await
        .map_err(|e| e.to_string())
}
