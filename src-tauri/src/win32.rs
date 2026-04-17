/// Win32 子窗口封装：为嵌入 MPV 视频渲染提供原生窗口句柄
///
/// 在 Tauri 主窗口的客户区创建一个黑色背景子窗口，MPV 通过 --wid 渲染视频于此。
use std::ffi::c_void;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, HGDIOBJ, BLACK_BRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, MoveWindow, RegisterClassW, SetWindowPos,
    HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WINDOW_EX_STYLE, WINDOW_STYLE,
    WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE, WS_EX_TRANSPARENT,
};

static CLASS_REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();

/// 将原始指针转换为 isize，用于跨线程/IPC 传递 HWND
#[inline]
fn hwnd_to_isize(h: HWND) -> isize {
    h.0 as isize
}

/// 将 isize 还原为 HWND
#[inline]
fn isize_to_hwnd(h: isize) -> HWND {
    HWND(h as *mut c_void)
}

/// 自定义窗口过程（等效 DefWindowProcW，满足 extern "system" 签名）
unsafe extern "system" fn video_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// 在 Tauri 主窗口内创建视频容器子窗口，返回 HWND 整数值
pub fn create_video_child_window(parent: isize, width: i32, height: i32) -> Result<isize, String> {
    unsafe {
        let instance = GetModuleHandleW(None)
            .map_err(|e| format!("GetModuleHandleW 失败：{}", e))?;

        // 只注册一次窗口类
        CLASS_REGISTERED.get_or_init(|| {
            let class_name = windows::core::w!("NetVidRew_Video");

            // GetStockObject 返回 HGDIOBJ，强转为 HBRUSH（相同的底层指针类型）
            let hgdi_obj: HGDIOBJ = GetStockObject(BLACK_BRUSH);
            let bg_brush = HBRUSH(hgdi_obj.0);

            let wc = WNDCLASSW {
                lpfnWndProc: Some(video_wnd_proc),
                hInstance: HINSTANCE(instance.0),
                hbrBackground: bg_brush,
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);
        });

        let class_name = windows::core::w!("NetVidRew_Video");
        let window_name = windows::core::w!("");
        let parent_hwnd = isize_to_hwnd(parent);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_TRANSPARENT.0), // 鼠标事件穿透到 WebView2
            class_name,
            window_name,
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_CLIPCHILDREN.0 | WS_CLIPSIBLINGS.0),
            0,
            0,
            width.max(1),
            height.max(1),
            Some(parent_hwnd),
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
        .map_err(|e| format!("CreateWindowExW 失败：{}", e))?;

        // 将视频窗口置于 z-order 顶部（覆盖在 WebView2 之上，WS_EX_TRANSPARENT 保证鼠标穿透）
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );

        Ok(hwnd_to_isize(hwnd))
    }
}

/// 调整视频子窗口的位置和大小（物理像素），并确保保持在 z-order 顶部
pub fn move_video_window(hwnd: isize, x: i32, y: i32, width: i32, height: i32) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let h = isize_to_hwnd(hwnd);
        let _ = MoveWindow(h, x, y, width.max(1), height.max(1), true);
        // 确保视频窗口始终在 WebView2 之上
        let _ = SetWindowPos(h, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}
