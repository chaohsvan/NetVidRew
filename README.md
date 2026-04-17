# NetVidRew

极简本地视频播放器。选一个目录，看完删掉，继续下一个。需要时还能快速剪出片段。

---

## 功能

- 扫描目录（含子目录）内所有视频文件，按文件名排序自动播放
- 播放 / 暂停、快进 / 快退（1/7 视频长度）、拖拽进度条跳转
- 音量调节
- **物理删除**当前视频文件（从硬盘直接删除，不经过回收站），自动播放下一个
- **视频剪辑**：标记入点 / 出点，一键调用 ffmpeg 导出片段（stream copy，秒级完成）
- 进度条上实时显示入/出点标记线及区间高亮
- 窗口可自由缩放，视频区域同步跟随

## 布局

```
┌──────────────────────────────┬──────────────┐
│                              │  ⬅ 入点  出点 ➡│
│                              │   ✂ 裁剪      │
│       视频播放区域            │  ⏪ 快退  ⏩ 快进│
│                              │   ▶ 播放      │
│                              │   🗑 删除     │
├──────────────────────────────┴──────────────┤
│  ▐██████████████░░░░░░░░░░░░▌  12:34 / 45:00│
│  🔊 ████████████████░░░░░░░░   80            │
└─────────────────────────────────────────────┘
```

## 依赖

运行本程序需要预先安装以下两个外部工具，并加入系统 `PATH`：

### MPV（视频播放）

- Windows：[mpv.io](https://mpv.io/installation/) 或 `scoop install mpv` / `winget install mpv`
- macOS：`brew install mpv`
- Linux：`sudo apt install mpv` / `sudo pacman -S mpv`

### FFmpeg（视频剪辑，可选）

仅使用剪辑功能时需要。

- Windows：[ffmpeg.org](https://ffmpeg.org/download.html) 或 `scoop install ffmpeg` / `winget install ffmpeg`
- macOS：`brew install ffmpeg`
- Linux：`sudo apt install ffmpeg` / `sudo pacman -S ffmpeg`

验证安装：

```bash
mpv --version
ffmpeg -version
```

## 使用

1. 启动程序，自动弹出目录选择框
2. 选择含视频的目录，第一个文件立即开始播放
3. 看完后点 **🗑 删除** 永久删除该文件并自动播放下一个

### 剪辑流程

1. 播放到目标片段起始位置，按 `I` 标记入点（进度条出现绿色标记线）
2. 播放到片段结束位置，按 `O` 标记出点（进度条出现红色标记线，区间高亮）
3. 按 `C` 或点击 **✂ 裁剪**，选择保存路径，ffmpeg 即刻完成导出

### 快捷键

| 按键 | 操作 |
|------|------|
| `Space` | 播放 / 暂停 |
| `←` | 快退（1/7 视频长度） |
| `→` | 快进（1/7 视频长度） |
| `↑` | 音量 +5 |
| `↓` | 音量 -5 |
| `I` | 标记入点 |
| `O` | 标记出点 |
| `C` | 裁剪并导出 |
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
        ├── state.rs            # 共享状态（AppState、PlaybackState）
        ├── commands.rs         # Tauri 命令（前端 invoke 的接口层）
        ├── utils.rs            # 工具函数（工具可用性检查、时间格式化）
        ├── mpv.rs              # MPV 进程控制 + IPC 通信
        ├── playlist.rs         # 播放列表（扫描目录、删除、索引管理）
        └── win32.rs            # Win32 API 封装（仅 Windows）
```

### Tauri 命令层（commands.rs）

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
| `clip_video` | 弹出保存对话框，调用 ffmpeg 裁剪并导出视频片段 |

### Win32 嵌入机制（win32.rs，仅 Windows）

```
Tauri 主窗口（父窗口）
├── WebView2 宿主窗口        ← UI 层（按钮、进度条、音量条）
└── 视频子窗口（Win32）      ← 覆盖在 WebView2 之上，MPV 渲染于此
    WS_EX_TRANSPARENT        ← 鼠标事件穿透回 WebView2，控件正常可点击
    HWND_TOP z-order         ← 视觉上覆盖 WebView2，显示真实视频画面
```

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
target/release/net-vid-rew.exe
target/release/bundle/msi/NetVidRew_0.1.0_x64_en-US.msi
target/release/bundle/nsis/NetVidRew_0.1.0_x64-setup.exe
```

## 技术栈

| 组件 | 说明 |
|------|------|
| Tauri 2.x | 跨平台桌面框架 |
| Rust 2021 | 后端逻辑 |
| WebView2 | 前端渲染（Windows 系统内置） |
| MPV | 外部依赖，视频播放 |
| FFmpeg | 外部依赖，视频剪辑（可选） |
| windows crate 0.61 | Win32 API 绑定 |

## 注意事项

- **删除操作不可撤销**，点击前**不会**弹出确认对话框
- **剪辑使用 stream copy 模式**，速度极快但精度受关键帧影响，实际剪切点可能偏移至最近的关键帧
- 视频子窗口使用物理像素坐标，高 DPI 缩放由 Win32 自动处理
- 当前仅在 Windows 上支持视频嵌入；macOS / Linux 降级为 MPV 独立窗口模式
