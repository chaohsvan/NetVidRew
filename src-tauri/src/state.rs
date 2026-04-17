use crate::mpv::MpvController;
use crate::playlist::Playlist;
use std::sync::Mutex;

/// 应用共享状态（跨 Tauri 命令传递）
pub struct AppState {
    pub playlist:    Mutex<Playlist>,
    pub mpv:         Mutex<Option<MpvController>>,
    /// 视频子窗口的 HWND 整数值（Windows 专用）
    pub video_hwnd:  Mutex<Option<isize>>,
}

/// 前端所需的播放状态快照
#[derive(serde::Serialize, Clone)]
pub struct PlaybackState {
    pub time_pos: f64,    // 当前播放位置（秒）
    pub duration: f64,    // 视频总时长（秒）
    pub paused:   bool,   // 是否暂停
    pub volume:   i64,    // 当前音量（0~100）
    pub filename: String, // 当前文件名
}
