use crate::mpv::MpvController;
use crate::playlist::Playlist;
use std::sync::Mutex;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

/// 应用共享状态
pub struct AppState {
    pub playlist: Mutex<Playlist>,
    pub mpv: Mutex<Option<MpvController>>,
    /// 视频子窗口的 HWND 整数值（Windows 专用）
    pub video_hwnd: Mutex<Option<isize>>,
}

/// 前端所需的播放状态快照
#[derive(serde::Serialize, Clone)]
pub struct PlaybackState {
    pub time_pos: f64,    // 当前播放位置（秒）
    pub duration: f64,    // 视频总时长（秒）
    pub paused: bool,     // 是否暂停
    pub volume: i64,      // 当前音量（0~100）
    pub filename: String, // 当前文件名
}

/// 检查 MPV 是否已安装
#[tauri::command]
pub fn check_mpv() -> Result<bool, String> {
    let ok = std::process::Command::new("mpv")
        .arg("--version")
        .output()
        .is_ok();
    Ok(ok)
}

/// 弹出目录选择框，扫描视频并启动 MPV 播放第一个
#[tauri::command]
pub async fn open_directory(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    // 弹出目录选择对话框（阻塞式）
    let folder = app.dialog().file().blocking_pick_folder();

    let dir_path = match folder {
        Some(p) => p.into_path().map_err(|e| format!("路径转换失败：{}", e))?,
        None => return Err("用户取消了目录选择".to_string()),
    };

    // 扫描视频文件
    let new_playlist = Playlist::from_dir(&dir_path)?;
    if new_playlist.is_empty() {
        return Err(format!("目录 {:?} 中未找到视频文件", dir_path));
    }

    let file_names = new_playlist.file_names();
    let first_file = new_playlist.current_file().unwrap().clone();

    // 更新播放列表
    {
        let mut pl = state.playlist.lock().map_err(|e| e.to_string())?;
        *pl = new_playlist;
    }

    // 读取视频子窗口 HWND
    let wid = {
        let hwnd_guard = state.video_hwnd.lock().map_err(|e| e.to_string())?;
        *hwnd_guard
    };
    eprintln!("[NetVidRew] open_directory: wid={:?}, file={:?}", wid, first_file.file_name());

    // 启动或复用 MPV
    {
        let mut mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
        let is_running = mpv_guard.as_mut().map(|c| c.is_running()).unwrap_or(false);
        if is_running {
            // MPV 已在运行，直接加载新文件
            mpv_guard.as_ref().unwrap().play_file(&first_file)?;
        } else {
            // 启动新 MPV 进程（嵌入到视频子窗口）
            let ctrl = MpvController::launch(Some(&first_file), wid)?;
            *mpv_guard = Some(ctrl);
        }
    }

    Ok(file_names)
}

/// 切换播放/暂停
#[tauri::command]
pub fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
    match mpv_guard.as_ref() {
        Some(ctrl) => ctrl.pause_toggle(),
        None => Err("MPV 未启动".to_string()),
    }
}

/// 相对跳转（秒，正数快进，负数快退）
#[tauri::command]
pub fn seek_relative(state: State<'_, AppState>, seconds: f64) -> Result<(), String> {
    let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
    match mpv_guard.as_ref() {
        Some(ctrl) => ctrl.seek(seconds, "relative"),
        None => Err("MPV 未启动".to_string()),
    }
}

/// 绝对定位（秒）
#[tauri::command]
pub fn seek_absolute(state: State<'_, AppState>, position: f64) -> Result<(), String> {
    let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
    match mpv_guard.as_ref() {
        Some(ctrl) => ctrl.seek(position, "absolute"),
        None => Err("MPV 未启动".to_string()),
    }
}

/// 设置音量（0~100）
#[tauri::command]
pub fn set_volume(state: State<'_, AppState>, volume: i64) -> Result<(), String> {
    let volume = volume.clamp(0, 100);
    let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
    match mpv_guard.as_ref() {
        Some(ctrl) => ctrl.set_volume(volume),
        None => Err("MPV 未启动".to_string()),
    }
}

/// 获取当前播放状态（前端每秒轮询）
#[tauri::command]
pub fn get_playback_state(state: State<'_, AppState>) -> Result<PlaybackState, String> {
    let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
    let ctrl = match mpv_guard.as_ref() {
        Some(c) => c,
        None => {
            return Ok(PlaybackState {
                time_pos: 0.0,
                duration: 0.0,
                paused: true,
                volume: 80,
                filename: String::new(),
            })
        }
    };

    let time_pos = ctrl
        .get_property("time-pos")
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let duration = ctrl
        .get_property("duration")
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let paused = ctrl
        .get_property("pause")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let volume = ctrl
        .get_property("volume")
        .ok()
        .and_then(|v| v.as_f64())
        .map(|v| v as i64)
        .unwrap_or(80);

    // 当前文件名
    let pl = state.playlist.lock().map_err(|e| e.to_string())?;
    let filename = pl
        .current_file()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(PlaybackState {
        time_pos,
        duration,
        paused,
        volume,
        filename,
    })
}

/// 删除当前视频文件并播放下一个；返回下一个文件名，None 表示列表已空
#[tauri::command]
pub fn delete_current(state: State<'_, AppState>) -> Result<Option<String>, String> {
    // 1. 停止 MPV 播放，释放文件占用
    {
        let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
        if let Some(ctrl) = mpv_guard.as_ref() {
            ctrl.stop()?;
        }
    }

    // 2. 从播放列表取出当前文件路径并物理删除
    let next_file = {
        let mut pl = state.playlist.lock().map_err(|e| e.to_string())?;
        let file_to_delete = pl.remove_current().ok_or("播放列表为空")?;

        std::fs::remove_file(&file_to_delete)
            .map_err(|e| format!("删除文件失败（{:?}）：{}", file_to_delete, e))?;

        pl.current_file().cloned()
    };

    // 3. 若还有下一个文件则播放
    if let Some(ref next) = next_file {
        let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
        if let Some(ctrl) = mpv_guard.as_ref() {
            ctrl.play_file(next)?;
        }
    }

    Ok(next_file.and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())))
}

/// 调整视频子窗口大小（由前端在窗口 resize 时调用，传入 CSS 像素坐标）
///
/// 注意：前端传来的是逻辑像素（CSS px），需要乘以设备像素比转换为物理像素。
/// 此命令仅在 Windows 上实际执行 Win32 操作，其他平台为空操作。
#[tauri::command]
pub fn resize_video(
    state: State<'_, AppState>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let hwnd_guard = state.video_hwnd.lock().map_err(|e| e.to_string())?;
        if let Some(hwnd) = *hwnd_guard {
            crate::win32::move_video_window(hwnd, x, y, width, height);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (state, x, y, width, height);
    }
    Ok(())
}
