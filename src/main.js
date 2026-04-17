// NetVidRew — 前端交互逻辑
// 通过 Tauri invoke 与 Rust 后端通信

const { invoke } = window.__TAURI__.core;

// ===== DOM 元素引用 =====
const videoArea      = document.getElementById('video-area');
const filenameDisplay = document.getElementById('filename-display');
const statusMsg      = document.getElementById('status-msg');
const progressBar    = document.getElementById('progress-bar');
const timeLabel      = document.getElementById('time-display');
const volumeBar      = document.getElementById('volume-bar');
const volumeLabel    = document.getElementById('volume-display');
const btnPlay        = document.getElementById('btn-play');
const btnRewind      = document.getElementById('btn-rewind');
const btnForward     = document.getElementById('btn-forward');
const btnDelete      = document.getElementById('btn-delete');

// ===== 前端状态 =====
let state = {
  duration:    0,
  currentTime: 0,
  volume:      80,
  isPaused:    true,
  isDragging:  false,
  pollTimer:   null,
  hasVideo:    false,
};

// ===== 工具函数 =====

function formatTime(s) {
  if (!isFinite(s) || s < 0) return '0:00';
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${sec.toString().padStart(2, '0')}`;
}

function showStatus(msg, type = '') {
  statusMsg.textContent = msg;
  statusMsg.className = 'show' + (type ? ' ' + type : '');
}

function clearStatus() {
  statusMsg.className = '';
  statusMsg.textContent = '';
}

/** 更新进度条渐变填充色 */
function updateProgressFill(value, max) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  progressBar.style.background =
    `linear-gradient(to right, var(--accent) ${pct}%, var(--surface) ${pct}%)`;
}

/** 应用来自 Rust 的播放状态 */
function applyPlaybackState(s) {
  state.duration    = s.duration  ?? 0;
  state.currentTime = s.time_pos  ?? 0;
  state.isPaused    = s.paused    ?? true;
  state.volume      = s.volume    ?? 80;
  state.hasVideo    = s.filename  !== '';

  // 进度条（拖拽时不覆盖）
  if (!state.isDragging) {
    const max = state.duration > 0 ? state.duration : 100;
    progressBar.max   = max;
    progressBar.value = state.currentTime;
    updateProgressFill(state.currentTime, max);
    timeLabel.textContent = `${formatTime(state.currentTime)} / ${formatTime(state.duration)}`;
  }

  // 音量
  volumeBar.value = state.volume;
  volumeLabel.textContent = state.volume;

  // 播放按钮文字
  btnPlay.textContent = state.isPaused ? '▶ 播放' : '⏸ 暂停';

  // 文件名显示区（右侧面板）
  if (s.filename) {
    filenameDisplay.textContent = s.filename;
    filenameDisplay.style.display = 'block';
    document.title = `NetVidRew — ${s.filename}`;
  } else {
    filenameDisplay.style.display = 'none';
    document.title = 'NetVidRew';
  }
}

// ===== 视频区域尺寸同步 =====
// 当视频区域大小变化时通知 Rust 调整 Win32 子窗口

let resizePending = false;

async function syncVideoSize() {
  if (resizePending) return;
  resizePending = true;
  // 延迟一帧确保 DOM 尺寸已稳定
  requestAnimationFrame(async () => {
    const rect = videoArea.getBoundingClientRect();
    try {
      await invoke('resize_video', {
        x: Math.round(rect.left),
        y: Math.round(rect.top),
        width:  Math.round(rect.width),
        height: Math.round(rect.height),
      });
    } catch (_) {}
    resizePending = false;
  });
}

const resizeObserver = new ResizeObserver(syncVideoSize);
resizeObserver.observe(videoArea);

// ===== 状态轮询 =====

async function pollPlaybackState() {
  try {
    const s = await invoke('get_playback_state');
    applyPlaybackState(s);
    if (state.duration > 0 && state.currentTime >= state.duration - 0.5) {
      showStatus('播放完毕', 'info');
    } else {
      clearStatus();
    }
  } catch (_) {
    // MPV 未就绪时静默忽略
  }
}

function startPolling() {
  if (state.pollTimer) return;
  state.pollTimer = setInterval(pollPlaybackState, 1000);
}

function stopPolling() {
  if (state.pollTimer) {
    clearInterval(state.pollTimer);
    state.pollTimer = null;
  }
}

// ===== 命令函数 =====

async function openDirectory() {
  showStatus('正在选择目录…', 'info');
  try {
    const files = await invoke('open_directory');
    if (files && files.length > 0) {
      state.hasVideo = true;
      showStatus(`已加载 ${files.length} 个视频`, 'info');
      startPolling();
      await syncVideoSize(); // 确保视频子窗口尺寸正确
      setTimeout(clearStatus, 2000);
    }
  } catch (e) {
    if (e && !e.includes('取消')) {
      showStatus(`⚠ ${e}`, 'error');
    } else {
      clearStatus();
    }
  }
}

async function togglePlayPause() {
  if (!state.hasVideo) {
    await openDirectory();
    return;
  }
  try {
    await invoke('play_pause');
    state.isPaused = !state.isPaused;
    btnPlay.textContent = state.isPaused ? '▶ 播放' : '⏸ 暂停';
  } catch (e) {
    showStatus(`⚠ ${e}`, 'error');
  }
}

async function seekRelative(secs) {
  if (!state.hasVideo) return;
  if (secs === 0) return;
  try {
    await invoke('seek_relative', { seconds: secs });
  } catch (e) {
    showStatus(`⚠ ${e}`, 'error');
  }
}

/** 跳转影片总长度的 1/7，方向由 sign (+1/-1) 决定 */
async function seekByFraction(sign) {
  if (!state.hasVideo || state.duration <= 0) return;
  const delta = (state.duration / 7) * sign;
  await seekRelative(delta);
}

async function seekAbsolute(position) {
  if (!state.hasVideo) return;
  try {
    await invoke('seek_absolute', { position });
  } catch (e) {
    showStatus(`⚠ ${e}`, 'error');
  }
}

async function changeVolume(volume) {
  try {
    await invoke('set_volume', { volume: parseInt(volume) });
    volumeLabel.textContent = volume;
  } catch (e) {
    showStatus(`⚠ ${e}`, 'error');
  }
}

async function deleteCurrentVideo() {
  if (!state.hasVideo) return;
  const confirmed = window.confirm(
    '确定要从硬盘上永久删除当前视频文件吗？\n此操作不可撤销！'
  );
  if (!confirmed) return;

  try {
    const nextFile = await invoke('delete_current');
    if (nextFile === null || nextFile === undefined) {
      state.hasVideo = false;
      filenameDisplay.style.display = 'none';
      document.title = 'NetVidRew';
      showStatus('所有视频已删除', 'info');
      stopPolling();
    } else {
      showStatus('已删除，播放下一个', 'info');
      setTimeout(clearStatus, 2000);
    }
  } catch (e) {
    showStatus(`⚠ 删除失败：${e}`, 'error');
  }
}

// ===== 事件绑定 =====

btnPlay.addEventListener('click',    togglePlayPause);
btnRewind.addEventListener('click',  () => seekByFraction(-1));
btnForward.addEventListener('click', () => seekByFraction(1));
btnDelete.addEventListener('click',  deleteCurrentVideo);

// 进度条拖拽
progressBar.addEventListener('mousedown', () => { state.isDragging = true; });
progressBar.addEventListener('input', (e) => {
  const val = parseFloat(e.target.value);
  updateProgressFill(val, state.duration || 100);
  timeLabel.textContent = `${formatTime(val)} / ${formatTime(state.duration)}`;
});
progressBar.addEventListener('change', async (e) => {
  state.isDragging = false;
  await seekAbsolute(parseFloat(e.target.value));
});
progressBar.addEventListener('mouseup', () => { state.isDragging = false; });

// 音量条
volumeBar.addEventListener('input', (e) => {
  volumeLabel.textContent = e.target.value;
});
volumeBar.addEventListener('change', (e) => {
  changeVolume(parseInt(e.target.value));
});

// 键盘快捷键
document.addEventListener('keydown', (e) => {
  switch (e.code) {
    case 'Space':      e.preventDefault(); togglePlayPause(); break;
    case 'ArrowRight': seekByFraction(1);  break;
    case 'ArrowLeft':  seekByFraction(-1); break;
    case 'ArrowUp':
      volumeBar.value = Math.min(100, parseInt(volumeBar.value) + 5);
      changeVolume(parseInt(volumeBar.value));
      break;
    case 'ArrowDown':
      volumeBar.value = Math.max(0, parseInt(volumeBar.value) - 5);
      changeVolume(parseInt(volumeBar.value));
      break;
    case 'Delete': deleteCurrentVideo(); break;
  }
});

// ===== 初始化 =====
window.addEventListener('DOMContentLoaded', async () => {
  // 初始同步视频区域尺寸（视频子窗口需对齐到 video-area）
  await syncVideoSize();

  // 检查 MPV 是否安装
  try {
    const mpvOk = await invoke('check_mpv');
    if (!mpvOk) {
      showStatus('⚠ 未找到 MPV，请安装 MPV 并加入 PATH', 'error');
      return;
    }
  } catch (_) {}

  // 自动弹出目录选择框
  await openDirectory();
});
