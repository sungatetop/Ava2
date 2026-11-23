//! ava2 Tauri 壳：app-host 原型的落地实践。
//!
//! 职责边界：
//! - 壳只做「窗口 + 边车生命周期 + 端口注出」，不含 Agent 逻辑；
//! - Agent 运行时由 `onion-server` 边车提供（HTTP/WS/SSE），前端经 `http://127.0.0.1:<port>` 消费。

mod sidecar;
mod update;

use std::sync::Arc;
use std::time::Duration;

use sidecar::Sidecar;
use tauri::RunEvent;

/// 边车就绪等待上限（首次启动需装配工具面与 MCP，留足余量）。
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// 前端查询边车基地址（`http://127.0.0.1:<port>`）。
#[tauri::command]
fn backend_base(state: tauri::State<'_, Arc<Sidecar>>) -> String {
    format!("http://127.0.0.1:{}", state.port())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let port = std::env::var("ONION_SERVER_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(sidecar::DEFAULT_PORT);
    let sidecar = Arc::new(Sidecar::new(port));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(sidecar.clone())
        .invoke_handler(tauri::generate_handler![
            backend_base,
            update::check_update,
            update::install_update
        ])
        .setup({
            let sidecar = sidecar.clone();
            move |app| {
                if let Err(error) = sidecar.start(app.handle()) {
                    // fail-loud：边车起不来时前端应看到明确错误，而不是静默空壳。
                    eprintln!("[ava2] 边车启动失败: {error}");
                } else if !sidecar::wait_ready(sidecar.port(), READY_TIMEOUT) {
                    eprintln!("[ava2] 边车端口 {} 在 {READY_TIMEOUT:?} 内未就绪", sidecar.port());
                }
                // 静默检查更新：有新版本时发事件给前端横幅（离线失败不打扰）
                update::spawn_check(app.handle().clone());
                Ok(())
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |_handle, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
            sidecar.stop();
        }
    });
}
