use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// IPC socket/pipe 路径（固定名称，单实例）
#[cfg(windows)]
fn ipc_path() -> String {
    r"\\.\pipe\mpvsocket-netvid".to_string()
}

#[cfg(unix)]
fn ipc_path() -> String {
    "/tmp/mpvsocket-netvid.sock".to_string()
}

/// MPV 进程控制器：负责启动子进程和 IPC 通信
pub struct MpvController {
    pub process: Child,
    pub socket_path: String,
}

impl MpvController {
    /// 启动 MPV 子进程
    ///
    /// - `file`: 可选的初始播放文件
    /// - `wid`:  嵌入模式下的目标窗口句柄（Win32 HWND），None 则独立窗口
    pub fn launch(file: Option<&Path>, wid: Option<isize>) -> Result<Self, String> {
        let socket_path = ipc_path();

        // 杀掉可能残留的同名 MPV 进程，避免 IPC 管道被占用
        kill_orphan_mpv();

        let mut cmd = Command::new("mpv");
        cmd.arg(format!("--input-ipc-server={}", socket_path))
            .arg("--idle=yes")
            .arg("--keep-open=yes")
            .arg("--no-terminal")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match wid {
            Some(handle) => {
                // 嵌入模式：渲染到指定的原生窗口，不创建独立窗口
                eprintln!("[NetVidRew] MPV launch: --wid={}", handle);
                cmd.arg(format!("--wid={}", handle))
                    .arg("--no-force-window");
            }
            None => {
                // 独立窗口模式（备用）
                eprintln!("[NetVidRew] MPV launch: 独立窗口模式（无 wid）");
                cmd.arg("--force-window=yes");
            }
        }

        if let Some(f) = file {
            cmd.arg(f.to_string_lossy().as_ref());
        }

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let child = cmd
            .spawn()
            .map_err(|e| format!("无法启动 MPV（请确认已安装并加入 PATH）：{}", e))?;

        let ctrl = MpvController {
            process: child,
            socket_path: socket_path.clone(),
        };

        // 等待 IPC socket 就绪（最多 15 次 × 200ms = 3s）
        ctrl.wait_for_socket(15)?;

        Ok(ctrl)
    }

    /// 等待 IPC socket 可连接
    fn wait_for_socket(&self, max_retries: u32) -> Result<(), String> {
        for i in 0..max_retries {
            thread::sleep(Duration::from_millis(200));
            if self.try_connect().is_ok() {
                return Ok(());
            }
            if i == max_retries - 1 {
                return Err("MPV IPC socket 连接超时".to_string());
            }
        }
        Ok(())
    }

    /// 尝试连接 IPC socket（用于检测是否就绪）
    fn try_connect(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            use std::fs::OpenOptions;
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.socket_path)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        #[cfg(unix)]
        {
            use std::os::unix::net::UnixStream;
            UnixStream::connect(&self.socket_path)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }

    /// 向 MPV IPC 发送 JSON 命令，返回响应的 data 字段
    pub fn send_command(&self, cmd: serde_json::Value) -> Result<serde_json::Value, String> {
        let msg = format!("{}\n", cmd);

        #[cfg(windows)]
        {
            self.send_command_windows(&msg)
        }
        #[cfg(unix)]
        {
            self.send_command_unix(&msg)
        }
    }

    #[cfg(windows)]
    fn send_command_windows(&self, msg: &str) -> Result<serde_json::Value, String> {
        use std::fs::OpenOptions;
        use std::io::{Read, Write};

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.socket_path)
            .map_err(|e| format!("连接 MPV IPC 失败：{}", e))?;

        file.write_all(msg.as_bytes())
            .map_err(|e| format!("发送 IPC 命令失败：{}", e))?;

        // 给 MPV 一点处理时间再读取响应
        thread::sleep(Duration::from_millis(50));

        let mut buf = [0u8; 4096];
        let response = match file.read(&mut buf) {
            Ok(n) if n > 0 => String::from_utf8_lossy(&buf[..n]).to_string(),
            _ => return Ok(serde_json::Value::Null),
        };

        // MPV 可能返回多行（事件 + 响应），找含 "error" 的行（即命令响应行）
        for line in response.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.get("error").is_some() {
                    let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
                    return Ok(data);
                }
            }
        }
        Ok(serde_json::Value::Null)
    }

    #[cfg(unix)]
    fn send_command_unix(&self, msg: &str) -> Result<serde_json::Value, String> {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| format!("连接 MPV IPC 失败：{}", e))?;
        stream
            .write_all(msg.as_bytes())
            .map_err(|e| format!("发送 IPC 命令失败：{}", e))?;

        let reader = std::io::BufReader::new(&stream);
        for line in reader.lines() {
            let line = line.map_err(|e| e.to_string())?;
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if v.get("error").is_some() {
                    let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
                    return Ok(data);
                }
            }
        }
        Ok(serde_json::Value::Null)
    }

    /// 播放指定文件
    pub fn play_file(&self, file: &Path) -> Result<(), String> {
        let path_str = file.to_string_lossy().to_string();
        self.send_command(serde_json::json!({
            "command": ["loadfile", path_str, "replace"]
        }))?;
        Ok(())
    }

    /// 切换播放/暂停
    pub fn pause_toggle(&self) -> Result<(), String> {
        self.send_command(serde_json::json!({
            "command": ["cycle", "pause"]
        }))?;
        Ok(())
    }

    /// 跳转（秒），mode: "relative" | "absolute"
    pub fn seek(&self, seconds: f64, mode: &str) -> Result<(), String> {
        self.send_command(serde_json::json!({
            "command": ["seek", seconds, mode]
        }))?;
        Ok(())
    }

    /// 设置音量（0~100）
    pub fn set_volume(&self, vol: i64) -> Result<(), String> {
        self.send_command(serde_json::json!({
            "command": ["set_property", "volume", vol]
        }))?;
        Ok(())
    }

    /// 获取 MPV 属性值
    pub fn get_property(&self, prop: &str) -> Result<serde_json::Value, String> {
        self.send_command(serde_json::json!({
            "command": ["get_property", prop]
        }))
    }

    /// 停止播放（保持 MPV 进程运行，idle 模式）
    pub fn stop(&self) -> Result<(), String> {
        self.send_command(serde_json::json!({
            "command": ["stop"]
        }))?;
        thread::sleep(Duration::from_millis(300));
        Ok(())
    }

    /// 退出 MPV 进程
    pub fn quit(&mut self) -> Result<(), String> {
        let _ = self.send_command(serde_json::json!({
            "command": ["quit"]
        }));
        thread::sleep(Duration::from_millis(200));
        let _ = self.process.kill();
        Ok(())
    }

    /// 检查 MPV 进程是否还在运行
    pub fn is_running(&mut self) -> bool {
        self.process
            .try_wait()
            .map(|s| s.is_none())
            .unwrap_or(false)
    }
}

/// 应用退出时自动 kill MPV 子进程，防止孤儿进程占用 IPC 管道
impl Drop for MpvController {
    fn drop(&mut self) {
        let _ = self.send_command(serde_json::json!({ "command": ["quit"] }));
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// 杀掉所有名为 mpv 的残留进程（跨平台）
fn kill_orphan_mpv() {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "mpv.exe"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        // 给 OS 一点时间释放管道
        thread::sleep(Duration::from_millis(200));
    }
    #[cfg(unix)]
    {
        let _ = Command::new("pkill")
            .arg("-f")
            .arg("mpv")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        thread::sleep(Duration::from_millis(200));
    }
}
