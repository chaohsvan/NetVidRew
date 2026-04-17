/// 检查外部工具（mpv / ffmpeg）是否已安装并存在于 PATH 中
pub fn is_tool_available(tool: &str) -> bool {
    std::process::Command::new(tool)
        .arg("--version")
        .output()
        .is_ok()
}

/// 将秒数格式化为 mmss 字符串（用于裁剪时的默认输出文件名）
pub fn format_seconds(secs: f64) -> String {
    let total = secs as u64;
    format!("{:02}{:02}", total / 60, total % 60)
}
