mod commands;
mod mpv;
mod playlist;

#[cfg(windows)]
mod win32;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager; // 提供 get_webview_window、state 等方法

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 检测 MPV 是否已安装（启动时提前检测，便于早期诊断）
    if std::process::Command::new("mpv")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("[NetVidRew] 错误：未找到 MPV，请安装 MPV 并加入 PATH");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            playlist: Mutex::new(playlist::Playlist::new()),
            mpv: Mutex::new(None),
            video_hwnd: Mutex::new(None),
        })
        .setup(|app| {
            // 在 Windows 上创建视频子窗口并嵌入 MPV
            #[cfg(windows)]
            {
                let win = app
                    .get_webview_window("main")
                    .ok_or("找不到主窗口")?;

                // 获取父窗口 HWND（windows 0.61 中 HWND 内部是 *mut c_void，转 isize）
                let parent_hwnd: isize = win.hwnd()?.0 as isize;

                // 获取窗口内部尺寸（物理像素）
                let size = win.inner_size()?;
                let video_w = (size.width as i32 - 216).max(1);
                let video_h = (size.height as i32 - 140).max(1);

                // 创建视频容器子窗口
                match win32::create_video_child_window(parent_hwnd, video_w, video_h) {
                    Ok(hwnd) => {
                        eprintln!("[NetVidRew] 视频子窗口创建成功，HWND={}, 尺寸={}x{}", hwnd, video_w, video_h);
                        // 存入 AppState，供 open_directory 和 resize_video 使用
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
            commands::open_directory,
            commands::play_pause,
            commands::seek_relative,
            commands::seek_absolute,
            commands::set_volume,
            commands::get_playback_state,
            commands::delete_current,
            commands::check_mpv,
            commands::resize_video,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
