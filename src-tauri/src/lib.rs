mod commands;
mod mpv;
mod playlist;
mod state;
mod utils;

#[cfg(windows)]
mod win32;

use state::AppState;
use std::sync::Mutex;
use tauri::Manager;

/// 右侧按钮栏的初始物理像素宽度（CSS 20% × 1280px 视口宽度，随后由前端 resize_video 动态同步）
const RIGHT_PANEL_WIDTH: i32 = 216;
/// 底部控制栏的初始物理像素高度（对应 CSS --ctrl-h: 130px）
const CONTROLS_HEIGHT: i32 = 130;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if !utils::is_tool_available("mpv") {
        eprintln!("[NetVidRew] 错误：未找到 MPV，请安装 MPV 并加入 PATH");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            playlist:    Mutex::new(playlist::Playlist::new()),
            mpv:         Mutex::new(None),
            video_hwnd:  Mutex::new(None),
        })
        .setup(|app| {
            #[cfg(windows)]
            {
                let win = app.get_webview_window("main").ok_or("找不到主窗口")?;
                let parent_hwnd: isize = win.hwnd()?.0 as isize;

                let size = win.inner_size()?;
                let video_w = (size.width  as i32 - RIGHT_PANEL_WIDTH).max(1);
                let video_h = (size.height as i32 - CONTROLS_HEIGHT).max(1);

                match win32::create_video_child_window(parent_hwnd, video_w, video_h) {
                    Ok(hwnd) => {
                        eprintln!("[NetVidRew] 视频子窗口创建成功，HWND={}, 尺寸={}x{}", hwnd, video_w, video_h);
                        *app.state::<AppState>().video_hwnd.lock().unwrap() = Some(hwnd);
                    }
                    Err(e) => {
                        eprintln!("[NetVidRew] 创建视频子窗口失败：{}", e);
                        // 非致命错误：降级为独立 MPV 窗口模式
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_mpv,
            commands::open_directory,
            commands::play_pause,
            commands::seek_relative,
            commands::seek_absolute,
            commands::set_volume,
            commands::get_playback_state,
            commands::navigate_next,
            commands::navigate_prev,
            commands::delete_current,
            commands::resize_video,
            commands::clip_video,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
