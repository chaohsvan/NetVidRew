// NetVidRew — 前端交互逻辑
// 通过 Tauri invoke 与 Rust 后端通信

const { invoke } = window.__TAURI__.core;

// ===== DOM 元素引用 =====
const videoArea       = document.getElementById('video-area');
const filenameDisplay = document.getElementById('filename-display');
const statusMsg       = document.getElementById('status-msg');
const progressBar     = document.getElementById('progress-bar');
const timeLabel       = document.getElementById('time-display');
const volumeBar       = document.getElementById('volume-bar');
const volumeLabel     = document.getElementById('volume-display');
const btnPlay         = document.getElementById('btn-play');
const btnRewind       = document.getElementById('btn-rewind');
const btnForward      = document.getElementById('btn-forward');
const btnPrev         = document.getElementById('btn-prev');
const btnNext         = document.getElementById('btn-next');
const btnDelete       = document.getElementById('btn-delete');
const btnMarkIn       = document.getElementById('btn-mark-in');
const btnMarkOut      = document.getElementById('btn-mark-out');
const btnClip         = document.getElementById('btn-clip');
const clipRange       = document.getElementById('clip-range');
const clipInMarker    = document.getElementById('clip-in-marker');
const clipOutMarker   = document.getElementById('clip-out-marker');

// ===== 统一应用状态 =====
const state = {
  // 播放
  duration:    0,
  currentTime: 0,
  volume:      80,
  isPaused:    true,
  hasVideo:    false,
  // 进度条拖拽
  isDragging:  false,
  // 轮询计时器
  pollTimer:   null,
  // 状态消息计时器
  statusTimer: null,
  // 裁剪入/出点（null 表示未标记）
  clipIn:  null,
  clipOut: null,
};

// ===== 工具函数 =====

function formatTime(s) {
  if (!isFinite(s) || s < 0) return '0:00';
  const m   = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${sec.toString().padStart(2, '0')}`;
}

/** 判断错误是否属于「用户主动取消」，取消不需要弹出错误提示 */
function isCancelled(e) {
  return e && String(e).includes('取消');
}

// ===== 状态消息 =====

function showStatus(msg, type = '', autoClear = 0) {
  statusMsg.textContent = msg;
  statusMsg.className   = 'show' + (type ? ' ' + type : '');
  if (state.statusTimer) clearTimeout(state.statusTimer);
  if (autoClear > 0) {
    state.statusTimer = setTimeout(clearStatus, autoClear);
  }
}

function clearStatus() {
  statusMsg.className   = '';
  statusMsg.textContent = '';
}

/** 统一错误处理：非取消操作才显示错误消息 */
function handleError(e, prefix = '') {
  if (isCancelled(e)) {
    clearStatus();
  } else {
    showStatus(`⚠ ${prefix}${e}`, 'error');
  }
}

// ===== 进度条渲染 =====

/** 通过 CSS 自定义属性驱动进度条渐变填充 */
function updateProgressFill(value, max) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  progressBar.style.setProperty('--progress-pct', `${pct}%`);
}

// ===== 裁剪入/出点标记 =====

function timeToPercent(t) {
  if (!state.duration || state.duration <= 0) return 0;
  return Math.max(0, Math.min(100, (t / state.duration) * 100));
}

function renderClipMarkers() {
  const hasIn  = state.clipIn  !== null;
  const hasOut = state.clipOut !== null;

  if (hasIn) {
    clipInMarker.style.left    = `${timeToPercent(state.clipIn)}%`;
    clipInMarker.style.display = 'block';
  } else {
    clipInMarker.style.display = 'none';
  }

  if (hasOut) {
    clipOutMarker.style.left    = `${timeToPercent(state.clipOut)}%`;
    clipOutMarker.style.display = 'block';
  } else {
    clipOutMarker.style.display = 'none';
  }

  const validRange = hasIn && hasOut && state.clipIn < state.clipOut;
  if (validRange) {
    const left  = timeToPercent(state.clipIn);
    const right = timeToPercent(state.clipOut);
    clipRange.style.left    = `${left}%`;
    clipRange.style.width   = `${right - left}%`;
    clipRange.style.display = 'block';
  } else {
    clipRange.style.display = 'none';
  }

  btnClip.disabled = !validRange;
}

function markIn() {
  if (!state.hasVideo) return;
  state.clipIn = state.currentTime;
  btnMarkIn.classList.add('marked');
  if (state.clipOut !== null && state.clipOut <= state.clipIn) {
    state.clipOut = null;
    btnMarkOut.classList.remove('marked');
  }
  renderClipMarkers();
  showStatus(`入点：${formatTime(state.clipIn)}`, 'info', 1500);
}

function markOut() {
  if (!state.hasVideo) return;
  state.clipOut = state.currentTime;
  btnMarkOut.classList.add('marked');
  if (state.clipIn !== null && state.clipIn >= state.clipOut) {
    state.clipIn = null;
    btnMarkIn.classList.remove('marked');
  }
  renderClipMarkers();
  showStatus(`出点：${formatTime(state.clipOut)}`, 'info', 1500);
}

function resetClipMarkers() {
  state.clipIn  = null;
  state.clipOut = null;
  btnMarkIn.classList.remove('marked');
  btnMarkOut.classList.remove('marked');
  renderClipMarkers();
}

// ===== 播放状态同步 =====

function applyPlaybackState(s) {
  state.duration    = s.duration  ?? 0;
  state.currentTime = s.time_pos  ?? 0;
  state.isPaused    = s.paused    ?? true;
  state.volume      = s.volume    ?? 80;
  state.hasVideo    = s.filename  !== '';

  if (!state.isDragging) {
    const max = state.duration > 0 ? state.duration : 100;
    progressBar.max   = max;
    progressBar.value = state.currentTime;
    updateProgressFill(state.currentTime, max);
    timeLabel.textContent = `${formatTime(state.currentTime)} / ${formatTime(state.duration)}`;
    renderClipMarkers();
  }

  volumeBar.value         = state.volume;
  volumeLabel.textContent = state.volume;
  btnPlay.textContent     = state.isPaused ? '▶ 播放' : '⏸ 暂停';

  if (s.filename) {
    filenameDisplay.textContent    = s.filename;
    filenameDisplay.style.display  = 'block';
    document.title = `NetVidRew — ${s.filename}`;
  } else {
    filenameDisplay.style.display  = 'none';
    document.title = 'NetVidRew';
  }
}

// ===== 视频区域尺寸同步 =====

let resizePending = false;

async function syncVideoSize() {
  if (resizePending) return;
  resizePending = true;
  requestAnimationFrame(async () => {
    const rect = videoArea.getBoundingClientRect();
    try {
      await invoke('resize_video', {
        x:      Math.round(rect.left),
        y:      Math.round(rect.top),
        width:  Math.round(rect.width),
        height: Math.round(rect.height),
      });
    } catch (_) {}
    resizePending = false;
  });
}

const resizeObserver = new ResizeObserver(syncVideoSize);
resizeObserver.observe(videoArea);

// 点击视频区域后焦点会丢到 Win32 子窗口，立即抢回到 body 保持快捷键可用
videoArea.addEventListener('mousedown', () => document.body.focus());

// ===== 状态轮询 =====

async function pollPlaybackState() {
  try {
    const s = await invoke('get_playback_state');
    applyPlaybackState(s);
    if (state.duration > 0 && state.currentTime >= state.duration - 0.5) {
      showStatus('播放完毕', 'info');
    } else if (statusMsg.textContent === '播放完毕') {
      clearStatus();
    }
  } catch (_) {}
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
      resetClipMarkers();
      showStatus(`已加载 ${files.length} 个视频`, 'info', 2000);
      startPolling();
      await syncVideoSize();
    }
  } catch (e) {
    handleError(e);
  }
}

async function togglePlayPause() {
  if (!state.hasVideo) {
    await openDirectory();
    return;
  }
  try {
    await invoke('play_pause');
    state.isPaused      = !state.isPaused;
    btnPlay.textContent = state.isPaused ? '▶ 播放' : '⏸ 暂停';
  } catch (e) {
    handleError(e);
  }
}

async function seekRelative(secs) {
  if (!state.hasVideo || secs === 0) return;
  try {
    await invoke('seek_relative', { seconds: secs });
  } catch (e) {
    handleError(e);
  }
}

async function seekByFraction(sign) {
  if (!state.hasVideo || state.duration <= 0) return;
  await seekRelative((state.duration / 7) * sign);
}

async function seekAbsolute(position) {
  if (!state.hasVideo) return;
  try {
    await invoke('seek_absolute', { position });
  } catch (e) {
    handleError(e);
  }
}

async function changeVolume(volume) {
  try {
    await invoke('set_volume', { volume: parseInt(volume) });
    volumeLabel.textContent = volume;
  } catch (e) {
    handleError(e);
  }
}

async function navigateNext() {
  if (!state.hasVideo) return;
  try {
    const nextFile = await invoke('navigate_next');
    resetClipMarkers();
    if (nextFile === null || nextFile === undefined) {
      showStatus('已是最后一个视频', 'info', 2000);
    } else {
      showStatus(`下一个：${nextFile}`, 'info', 2000);
    }
  } catch (e) {
    handleError(e, '切换失败：');
  }
}

async function navigatePrev() {
  if (!state.hasVideo) return;
  try {
    const prevFile = await invoke('navigate_prev');
    resetClipMarkers();
    if (prevFile === null || prevFile === undefined) {
      showStatus('已是第一个视频', 'info', 2000);
    } else {
      showStatus(`上一个：${prevFile}`, 'info', 2000);
    }
  } catch (e) {
    handleError(e, '切换失败：');
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
    resetClipMarkers();
    if (nextFile === null || nextFile === undefined) {
      state.hasVideo                = false;
      filenameDisplay.style.display = 'none';
      document.title                = 'NetVidRew';
      showStatus('所有视频已删除', 'info');
      stopPolling();
    } else {
      showStatus('已删除，播放下一个', 'info', 2000);
    }
  } catch (e) {
    handleError(e, '删除失败：');
  }
}

async function clipVideo() {
  if (!state.hasVideo || state.clipIn === null || state.clipOut === null) return;
  if (state.clipIn >= state.clipOut) {
    showStatus('⚠ 入点必须早于出点', 'error', 2500);
    return;
  }

  showStatus('正在导出…', 'info');
  try {
    const result = await invoke('clip_video', {
      startSec: state.clipIn,
      endSec:   state.clipOut,
    });
    if (result) {
      showStatus(`✔ 裁剪完成：${result}`, 'success', 4000);
    } else {
      clearStatus();
    }
  } catch (e) {
    handleError(e, '裁剪失败：');
  }
}

// ===== 事件绑定 =====

btnPlay.addEventListener('click',    togglePlayPause);
btnRewind.addEventListener('click',  () => seekByFraction(-1));
btnForward.addEventListener('click', () => seekByFraction(1));
btnPrev.addEventListener('click',    navigatePrev);
btnNext.addEventListener('click',    navigateNext);
btnDelete.addEventListener('click',  deleteCurrentVideo);
btnMarkIn.addEventListener('click',  markIn);
btnMarkOut.addEventListener('click', markOut);
btnClip.addEventListener('click',    clipVideo);

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
volumeBar.addEventListener('input',  (e) => { volumeLabel.textContent = e.target.value; });
volumeBar.addEventListener('change', (e) => { changeVolume(parseInt(e.target.value)); });

// 键盘快捷键
document.addEventListener('keydown', (e) => {
  if (e.target.tagName === 'INPUT') return;
  switch (e.code) {
    case 'Space':      e.preventDefault(); togglePlayPause();   break;
    case 'ArrowRight': e.preventDefault(); seekByFraction(1);   break;
    case 'ArrowLeft':  e.preventDefault(); seekByFraction(-1);  break;
    case 'ArrowUp':
      e.preventDefault();
      volumeBar.value = Math.min(100, parseInt(volumeBar.value) + 5);
      changeVolume(parseInt(volumeBar.value));
      break;
    case 'ArrowDown':
      e.preventDefault();
      volumeBar.value = Math.max(0, parseInt(volumeBar.value) - 5);
      changeVolume(parseInt(volumeBar.value));
      break;
    case 'Delete':     deleteCurrentVideo();  break;
    case 'BracketLeft':  navigatePrev();      break;  // [
    case 'BracketRight': navigateNext();      break;  // ]
    case 'KeyI':   markIn();             break;
    case 'KeyO':   markOut();            break;
    case 'KeyC':   clipVideo();          break;
  }
});

// ===== 初始化 =====
window.addEventListener('DOMContentLoaded', async () => {
  btnClip.disabled = true;
  // 页面加载后立即抢占焦点，确保键盘快捷键无需先点击页面
  document.body.setAttribute('tabindex', '-1');
  document.body.focus();
  await syncVideoSize();

  try {
    const mpvOk = await invoke('check_mpv');
    if (!mpvOk) {
      showStatus('⚠ 未找到 MPV，请安装 MPV 并加入 PATH', 'error');
      return;
    }
  } catch (_) {}

  await openDirectory();
});
