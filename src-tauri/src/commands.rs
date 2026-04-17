use crate::mpv::MpvController;
use crate::state::{AppState, PlaybackState};
use crate::utils;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

/// 检查 MPV 是否已安装
#[tauri::command]
pub fn check_mpv() -> Result<bool, String> {
    Ok(utils::is_tool_available("mpv"))
}

/// 弹出目录选择框，扫描视频并启动 MPV 播放第一个
#[tauri::command]
pub async fn open_directory(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let folder = app.dialog().file().blocking_pick_folder();

    let dir_path = match folder {
        Some(p) => p.into_path().map_err(|e| format!("路径转换失败：{}", e))?,
        None    => return Err("用户取消了目录选择".to_string()),
    };

    let new_playlist = crate::playlist::Playlist::from_dir(&dir_path)?;
    if new_playlist.is_empty() {
        return Err(format!("目录 {:?} 中未找到视频文件", dir_path));
    }

    let file_names  = new_playlist.file_names();
    let first_file  = new_playlist.current_file().unwrap().clone();

    {
        let mut pl = state.playlist.lock().map_err(|e| e.to_string())?;
        *pl = new_playlist;
    }

    let wid = {
        let guard = state.video_hwnd.lock().map_err(|e| e.to_string())?;
        *guard
    };
    eprintln!("[NetVidRew] open_directory: wid={:?}, file={:?}", wid, first_file.file_name());

    {
        let mut mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
        let is_running = mpv_guard.as_mut().map(|c| c.is_running()).unwrap_or(false);
        if is_running {
            mpv_guard.as_ref().unwrap().play_file(&first_file)?;
        } else {
            *mpv_guard = Some(MpvController::launch(Some(&first_file), wid)?);
        }
    }

    Ok(file_names)
}

/// 切换播放/暂停
#[tauri::command]
pub fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    with_mpv(&state, |ctrl| ctrl.pause_toggle())
}

/// 相对跳转（秒，正数快进，负数快退）
#[tauri::command]
pub fn seek_relative(state: State<'_, AppState>, seconds: f64) -> Result<(), String> {
    with_mpv(&state, |ctrl| ctrl.seek(seconds, "relative"))
}

/// 绝对定位（秒）
#[tauri::command]
pub fn seek_absolute(state: State<'_, AppState>, position: f64) -> Result<(), String> {
    with_mpv(&state, |ctrl| ctrl.seek(position, "absolute"))
}

/// 设置音量（0~100）
#[tauri::command]
pub fn set_volume(state: State<'_, AppState>, volume: i64) -> Result<(), String> {
    with_mpv(&state, |ctrl| ctrl.set_volume(volume.clamp(0, 100)))
}

/// 获取当前播放状态（前端每秒轮询）
#[tauri::command]
pub fn get_playback_state(state: State<'_, AppState>) -> Result<PlaybackState, String> {
    let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;

    let Some(ctrl) = mpv_guard.as_ref() else {
        return Ok(PlaybackState {
            time_pos: 0.0,
            duration: 0.0,
            paused:   true,
            volume:   80,
            filename: String::new(),
        });
    };

    let time_pos = ctrl.get_property("time-pos").ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let duration = ctrl.get_property("duration").ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let paused   = ctrl.get_property("pause").ok().and_then(|v| v.as_bool()).unwrap_or(false);
    let volume   = ctrl.get_property("volume").ok().and_then(|v| v.as_f64()).map(|v| v as i64).unwrap_or(80);

    let pl = state.playlist.lock().map_err(|e| e.to_string())?;
    let filename = pl
        .current_file()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(PlaybackState { time_pos, duration, paused, volume, filename })
}

/// 删除当前视频文件并播放下一个；返回下一个文件名，None 表示列表已空
#[tauri::command]
pub fn delete_current(state: State<'_, AppState>) -> Result<Option<String>, String> {
    {
        let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
        if let Some(ctrl) = mpv_guard.as_ref() {
            ctrl.stop()?;
        }
    }

    let next_file = {
        let mut pl = state.playlist.lock().map_err(|e| e.to_string())?;
        let to_delete = pl.remove_current().ok_or("播放列表为空")?;
        std::fs::remove_file(&to_delete)
            .map_err(|e| format!("删除文件失败（{:?}）：{}", to_delete, e))?;
        pl.current_file().cloned()
    };

    if let Some(ref next) = next_file {
        let mpv_guard = state.mpv.lock().map_err(|e| e.to_string())?;
        if let Some(ctrl) = mpv_guard.as_ref() {
            ctrl.play_file(next)?;
        }
    }

    Ok(next_file.and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())))
}

/// 调整视频子窗口大小（前端在窗口 resize 时调用，传入 CSS 逻辑像素坐标）
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
        let guard = state.video_hwnd.lock().map_err(|e| e.to_string())?;
        if let Some(hwnd) = *guard {
            crate::win32::move_video_window(hwnd, x, y, width, height);
        }
    }
    #[cfg(not(windows))]
    let _ = (state, x, y, width, height);
    Ok(())
}

/// 裁剪当前视频：弹出保存路径对话框，调用 ffmpeg 导出片段
///
/// - `start_sec`：入点（秒）
/// - `end_sec`：  出点（秒）
///
/// 返回输出文件名（仅文件名部分），用户取消则返回 `None`。
#[tauri::command]
pub async fn clip_video(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    start_sec: f64,
    end_sec: f64,
) -> Result<Option<String>, String> {
    if start_sec >= end_sec {
        return Err("入点必须早于出点".to_string());
    }

    let src_path = {
        let pl = state.playlist.lock().map_err(|e| e.to_string())?;
        pl.current_file().cloned().ok_or_else(|| "播放列表为空".to_string())?
    };

    let ext  = src_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_string();
    let stem = src_path.file_stem().and_then(|s| s.to_str()).unwrap_or("clip");
    let default_name = format!(
        "{}_clip_{}-{}.{}",
        stem,
        utils::format_seconds(start_sec),
        utils::format_seconds(end_sec),
        ext
    );

    let save_path = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .add_filter("视频文件", &[&ext])
        .blocking_save_file();

    let out_path = match save_path {
        Some(p) => p.into_path().map_err(|e| format!("路径转换失败：{}", e))?,
        None    => return Ok(None),
    };

    if !utils::is_tool_available("ffmpeg") {
        return Err("未找到 ffmpeg，请安装 ffmpeg 并加入 PATH".to_string());
    }

    // 在独立的阻塞线程池中运行 ffmpeg，避免占用 Tokio async worker 线程
    // 注意：-ss 在 -i 前面时使用 fast seek，此时 -to 指向输出流时间戳（非相对入点），
    // 必须用 -t <duration> 才能精确截取目标片段。
    let duration = end_sec - start_sec;
    let out_path_clone = out_path.clone();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new("ffmpeg")
            .arg("-y")
            .arg("-ss").arg(start_sec.to_string())
            .arg("-i").arg(&src_path)
            .arg("-t").arg(duration.to_string())
            .arg("-c").arg("copy")
            .arg("-avoid_negative_ts").arg("make_zero")
            .arg(&out_path_clone)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    })
    .await
    .map_err(|e| format!("裁剪任务异常：{}", e))?
    .map_err(|e| format!("启动 ffmpeg 失败：{}", e))?;

    if !status.success() {
        return Err(format!("ffmpeg 裁剪失败（退出码：{:?}）", status.code()));
    }

    let out_name = out_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| out_path.to_string_lossy().to_string());

    Ok(Some(out_name))
}

// ===== 内部辅助 =====

/// 从 AppState 取出 MPV 控制器并执行操作；MPV 未启动时返回错误。
fn with_mpv<F>(state: &State<'_, AppState>, f: F) -> Result<(), String>
where
    F: FnOnce(&MpvController) -> Result<(), String>,
{
    let guard = state.mpv.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(ctrl) => f(ctrl),
        None       => Err("MPV 未启动".to_string()),
    }
}
