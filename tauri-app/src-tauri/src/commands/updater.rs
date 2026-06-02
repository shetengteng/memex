use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// 是否有可用更新。如果当前已经是最新版本，available 为 false，其他字段为 None。
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    /// release note（来自 latest.json 的 notes 字段）。
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateInfo, String> {
    let current_version = app.package_info().version.to_string();

    let updater = app
        .updater()
        .map_err(|e| format!("初始化 updater 失败：{}", e))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{}", e))?;

    match update {
        Some(u) => Ok(UpdateInfo {
            available: true,
            current_version,
            latest_version: Some(u.version.clone()),
            notes: u.body.clone(),
        }),
        None => Ok(UpdateInfo {
            available: false,
            current_version,
            latest_version: None,
            notes: None,
        }),
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("初始化 updater 失败：{}", e))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{}", e))?
        .ok_or_else(|| "当前已经是最新版本，没有需要安装的更新".to_string())?;

    // download_and_install 会下载 .app.tar.gz / .dmg，验证签名后替换当前 .app，并自动重启。
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("下载或安装更新失败：{}", e))?;

    // 控制权在 download_and_install 内部就被新进程接管了，这里通常不会执行到。
    Ok(())
}
