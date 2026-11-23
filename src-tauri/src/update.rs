//! 桌面端自动更新（tauri-plugin-updater）。
//!
//! 链路：启动静默检查 → 有新版本经事件 `ava2://update-available` 通知前端 → 用户确认后
//! 下载校验（minisign 签名由插件校验）→ 停边车 → 安装 → 重启。
//!
//! 更新清单由 `scripts/publish.ps1` 生成并上传 OSS：
//! `ava2/channels/<channel>/windows-x86_64/update.json`；
//! 签名私钥 `scripts/keys/ava2-updater.key`（不入库），公钥已编入客户端 `tauri.conf.json`。

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

use crate::sidecar::Sidecar;

/// 前端监听的事件名：发现新版本。
pub const UPDATE_EVENT: &str = "ava2://update-available";

/// 可更新版本信息（前端横幅展示）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// 启动后台检查：有新版本时向所有窗口发事件。
///
/// 失败静默（离线 / OSS 不可达时不打扰用户），仅写 stderr 便于排查。
pub fn spawn_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok(updater) = app.updater() else {
            return;
        };
        match updater.check().await {
            Ok(Some(update)) => {
                let info = UpdateInfo {
                    version: update.version.clone(),
                    current_version: update.current_version.clone(),
                    notes: update.body.clone(),
                };
                if let Err(error) = app.emit(UPDATE_EVENT, info) {
                    eprintln!("[ava2] 更新事件派发失败: {error}");
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("[ava2] 更新检查失败: {error}"),
        }
    });
}

/// 手动检查更新（前端可主动拉取，无需等待启动检查）。
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    let update = updater.check().await.map_err(|error| error.to_string())?;
    Ok(update.map(|update| UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
    }))
}

/// 下载并安装更新，然后重启。
///
/// **必须先停边车**：Windows 安装器要覆盖 `onion-server.exe` / `onion-sandbox.exe`，
/// 运行中的进程会让覆盖失败导致安装中止。
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    sidecar: tauri::State<'_, Arc<Sidecar>>,
) -> Result<(), String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "已是最新版本".to_string())?;
    sidecar.stop();
    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|error| format!("下载或安装失败：{error}"))?;
    // restart 为发散返回（`!`），无需再补 return
    app.restart()
}
