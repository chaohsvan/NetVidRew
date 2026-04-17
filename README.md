# NetVidRew

极简本地视频播放器。选一个目录，看完删掉，继续下一个。

---

## 功能

- 扫描目录（含子目录）内所有视频文件，按文件名排序自动播放
- 播放 / 暂停、快进 / 快退 10 秒、拖拽进度条跳转
- 音量调节
- **物理删除**当前视频文件（从硬盘直接删除，不经过回收站），自动播放下一个
- 窗口可自由缩放，视频区域同步跟随

## 截图

```
┌──────────────────────────────────┬────────────┐
│                                  │  ⏪ 快退   │
│                                  │  ▶ 播放    │
│         视频播放区域              │  ⏩ 快进   │
│                                  │  🗑 删除   │
├──────────────────────────────────┴────────────┤
│  [══════════════●══════] 12:34 / 45:00        │
│  🔊 [════════════●═════] 80                   │
└───────────────────────────────────────────────┘
```

## 依赖

运行本程序需要预先安装 **MPV**：

- Windows：[mpv.io](https://mpv.io/installation/) 或 `scoop install mpv` / `winget install mpv`
- macOS：`brew install mpv`
- Linux：`sudo apt install mpv` / `sudo pacman -S mpv`

安装后确保 `mpv` 已加入系统 `PATH`（在终端执行 `mpv --version` 能输出版本号即可）。

## 使用

1. 启动程序，自动弹出目录选择框
2. 选择含视频的目录，第一个文件立即开始播放
3. 看完后点 **🗑 删除** 永久删除该文件并自动播放下一个

### 快捷键

| 按键 | 操作 |
|------|------|
| `Space` | 播放 / 暂停 |
| `←` | 快退 10 秒 |
| `→` | 快进 10 秒 |
| `↑` | 音量 +5 |
| `↓` | 音量 -5 |
| `Delete` | 删除当前视频 |

### 支持的视频格式

`mp4` `mkv` `avi` `mov` `wmv` `flv` `webm` `ts` `m2ts` `rmvb` `m4v` `mpg` `mpeg` `3gp` `ogv`

（MPV 本身支持几乎所有格式，扩展名列表仅用于目录扫描过滤）

---

## 项目架构

```
NetVidRew/
├── src/                        # 前端（原生 Web，无框架）
│   ├── index.html              # UI 结构
│   ├── style.css               # 样式
│   └── main.js                 # 交互逻辑 + Tauri IPC 调用
│
└── src-tauri/                  # Rust 后端（Tauri）
    ├── tauri.conf.json         # 应用配置（窗口尺寸、透明、打包）
    ├── Cargo.toml              # 依赖声明
    └── src/
        ├── main.rs             # 程序入口
        ├── lib.rs              # Tauri 初始化、Win32 子窗口创建
        ├── commands.rs         # Tauri 命令（前端 invoke 的接口层）
        ├── mpv.rs              # MPV 进程控制 + IPC 通信
        ├── playlist.rs         # 播放列表（扫描目录、删除、索引管理）
        └── win32.rs            # Win32 API 封装（仅 Windows）
```

### 各层职责

**前端（HTML/CSS/JS）**
- 纯原生 Web，无任何 npm 依赖
- 通过 `window.__TAURI__.core.invoke` 调用 Rust 命令
- `ResizeObserver` 监听 `#video-area` 尺寸变化，实时通知 Rust 同步视频窗口大小
- 每秒轮询 `get_playback_state` 刷新进度条、时间显示、音量

**Tauri 命令层（commands.rs）**

| 命令 | 说明 |
|------|------|
| `open_directory` | 弹出目录选择框，扫描视频，启动或复用 MPV |
| `play_pause` | 切换播放/暂停 |
| `seek_relative` | 相对跳转（秒） |
| `seek_absolute` | 绝对定位（秒） |
| `set_volume` | 设置音量 0~100 |
| `get_playback_state` | 返回当前时间、总时长、暂停状态、音量、文件名 |
| `delete_current` | 停止播放 → 物理删除文件 → 播放下一个 |
| `resize_video` | 调整视频 Win32 子窗口的位置和大小 |
| `check_mpv` | 检测 MPV 是否已安装 |

**MPV 控制层（mpv.rs）**
- 以子进程方式启动 MPV，使用固定命名管道 `\\.\pipe\mpvsocket-netvid` 进行 IPC
- 通过 JSON 命令协议（MPV IPC 标准格式）控制播放、跳转、音量、获取属性
- `--wid=<HWND>` 参数将视频渲染嵌入到指定的 Win32 窗口句柄

**Win32 嵌入层（win32.rs，仅 Windows）**

视频嵌入的核心机制：

```
Tauri 主窗口（父窗口）
├── WebView2 宿主窗口        ← UI 层（按钮、进度条、音量条）
└── 视频子窗口（Win32）      ← 覆盖在 WebView2 之上，MPV 渲染于此
    WS_EX_TRANSPARENT        ← 鼠标事件穿透回 WebView2，控件正常可点击
    HWND_TOP z-order         ← 视觉上覆盖 WebView2，显示真实视频画面
```

- `create_video_child_window`：注册窗口类（黑色背景），创建带 `WS_EX_TRANSPARENT` 的子窗口，置于 `HWND_TOP`
- `move_video_window`：调整子窗口位置/大小（物理像素），每次同时刷新 z-order 防止 Tauri 内部重排

**播放列表（playlist.rs）**
- 递归扫描目录，按文件名排序
- 维护当前播放索引，删除时自动调整到下一个合法位置

---

## 构建

```bash
# 开发模式（热重载）
cargo tauri dev

# 生产构建
cargo tauri build
```

构建产物：

```
target/release/net-vid-rew.exe                        # 裸 EXE
target/release/bundle/msi/NetVidRew_0.1.0_x64_en-US.msi
target/release/bundle/nsis/NetVidRew_0.1.0_x64-setup.exe
```

## 技术栈

| 组件 | 版本 |
|------|------|
| Tauri | 2.x |
| Rust | edition 2021 |
| WebView2 | 系统内置（Windows 11 自带） |
| MPV | 外部依赖，需用户自行安装 |
| windows crate | 0.61（Win32 API 绑定） |

## 注意事项

- **删除操作不可撤销**，点击删除前会弹出确认对话框
- 视频子窗口使用物理像素坐标，高 DPI 缩放由 Win32 自动处理
- 当前仅在 Windows 上支持视频嵌入；macOS / Linux 降级为 MPV 独立窗口模式
