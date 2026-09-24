use crate::{
    application::{desktop_actions::QuitState, ocr_actions},
    features::pins,
    infrastructure::{
        desktop_windows::repository::{self, WindowState},
        persistence::Database,
    },
};
use image::{imageops::FilterType, ImageBuffer, Rgba, RgbaImage};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc, Mutex, OnceLock},
    time::Duration,
};
use tauri::{Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
        Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, AC_SRC_ALPHA,
            AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS,
            HBITMAP, HDC, HGDIOBJ,
        },
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            Input::KeyboardAndMouse::{GetKeyState, ReleaseCapture, VK_CONTROL},
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
                DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowLongPtrW,
                GetWindowRect, KillTimer, LoadCursorW, PeekMessageW, PostThreadMessageW,
                RegisterClassExW, SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
                SetWindowPos, ShowWindow, TrackPopupMenu, TranslateMessage, UpdateLayeredWindow,
                CREATESTRUCTW, CS_DBLCLKS, GWLP_USERDATA, HTCAPTION, HWND_TOPMOST, IDC_ARROW,
                MF_SEPARATOR, MF_STRING, MSG, PM_NOREMOVE, SET_WINDOW_POS_FLAGS, SWP_NOMOVE,
                SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOW, TPM_RETURNCMD, TPM_RIGHTBUTTON, ULW_ALPHA,
                WM_APP, WM_CONTEXTMENU, WM_EXITSIZEMOVE, WM_KEYDOWN, WM_LBUTTONDBLCLK,
                WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_NCLBUTTONDBLCLK,
                WM_NCLBUTTONDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST, WS_POPUP,
            },
        },
    },
};

const COMMAND_MESSAGE: u32 = WM_APP + 41;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const MENU_COPY: i32 = 1;
const MENU_OCR: i32 = 2;
const MENU_RESET: i32 = 3;
const MENU_CLOSE: i32 = 4;
const ZOOM_SETTLE_TIMER: usize = 1;
const ZOOM_SETTLE_MS: u32 = 160;
const MIN_ZOOM: f64 = 0.1;
const MAX_ZOOM: f64 = 3.0;
const MAX_RENDER_DIMENSION: u32 = 8192;
const MAX_RENDER_PIXELS: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderQuality {
    Fast,
    High,
}

#[derive(Clone)]
pub struct PinWindowSpec {
    pub id: String,
    pub workspace_id: String,
    pub image_id: String,
    pub image_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub zoom: f64,
    pub position: Option<(i32, i32)>,
    pub saved: Option<WindowState>,
}

enum Command {
    Open {
        spec: PinWindowSpec,
        reply: mpsc::SyncSender<Result<(), String>>,
    },
    Close {
        id: String,
        reply: mpsc::SyncSender<Result<(), String>>,
    },
    PersistAll {
        reply: mpsc::SyncSender<Result<(), String>>,
    },
}

struct PinThread {
    id: u32,
}

static PIN_THREAD: OnceLock<PinThread> = OnceLock::new();
static PIN_THREAD_INIT: Mutex<()> = Mutex::new(());

thread_local! {
    static WINDOWS: RefCell<HashMap<String, HWND>> = RefCell::new(HashMap::new());
}

pub fn open(app: &tauri::AppHandle, spec: PinWindowSpec) -> Result<(), String> {
    call(app, |reply| Command::Open { spec, reply })
}

pub fn close(app: &tauri::AppHandle, id: String) -> Result<(), String> {
    call(app, |reply| Command::Close { id, reply })
}

pub fn persist_all(app: &tauri::AppHandle) -> Result<(), String> {
    if PIN_THREAD.get().is_none() {
        return Ok(());
    }
    call(app, |reply| Command::PersistAll { reply })
}

fn call(
    app: &tauri::AppHandle,
    build: impl FnOnce(mpsc::SyncSender<Result<(), String>>) -> Command,
) -> Result<(), String> {
    let thread = ensure_thread(app)?;
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    let command = Box::new(build(reply_tx));
    let pointer = Box::into_raw(command);
    if let Err(error) = unsafe {
        PostThreadMessageW(
            thread.id,
            COMMAND_MESSAGE,
            WPARAM(0),
            LPARAM(pointer as isize),
        )
    } {
        unsafe {
            drop(Box::from_raw(pointer));
        }
        return Err(format!("唤醒原生贴图线程失败: {error}"));
    }
    reply_rx
        .recv_timeout(COMMAND_TIMEOUT)
        .map_err(|_| "原生贴图窗口操作超时".to_string())?
}

fn ensure_thread(app: &tauri::AppHandle) -> Result<&'static PinThread, String> {
    if let Some(thread) = PIN_THREAD.get() {
        return Ok(thread);
    }
    let _guard = PIN_THREAD_INIT
        .lock()
        .map_err(|_| "原生贴图线程初始化锁不可用")?;
    if let Some(thread) = PIN_THREAD.get() {
        return Ok(thread);
    }

    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let handle = app.clone();
    std::thread::Builder::new()
        .name("ww-ocr-native-pins".into())
        .spawn(move || pin_thread(handle, ready_tx))
        .map_err(|error| format!("创建原生贴图线程失败: {error}"))?;
    let id = ready_rx
        .recv_timeout(COMMAND_TIMEOUT)
        .map_err(|_| "原生贴图线程启动超时".to_string())??;
    let _ = PIN_THREAD.set(PinThread { id });
    PIN_THREAD.get().ok_or("原生贴图线程不可用".into())
}

fn pin_thread(app: tauri::AppHandle, ready: mpsc::SyncSender<Result<u32, String>>) {
    let result = unsafe { register_window_class() };
    if let Err(error) = result {
        let _ = ready.send(Err(error));
        return;
    }

    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));

    loop {
        let status = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if status.0 <= 0 {
            break;
        }
        if message.message == COMMAND_MESSAGE {
            let command = unsafe { Box::from_raw(message.lParam.0 as *mut Command) };
            handle_command(&app, *command);
        } else {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

unsafe fn register_window_class() -> Result<(), String> {
    let module = GetModuleHandleW(None).map_err(|error| error.to_string())?;
    let cursor = LoadCursorW(None, IDC_ARROW).map_err(|error| error.to_string())?;
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_DBLCLKS,
        lpfnWndProc: Some(window_proc),
        hInstance: HINSTANCE(module.0),
        hCursor: cursor,
        lpszClassName: w!("WWOcrNativePinWindow"),
        ..Default::default()
    };
    if RegisterClassExW(&class) == 0 {
        return Err("注册原生贴图窗口类失败".into());
    }
    Ok(())
}

fn handle_command(app: &tauri::AppHandle, command: Command) {
    match command {
        Command::Open { spec, reply } => {
            let result = WINDOWS.with(|windows| {
                if let Some(hwnd) = windows.borrow().get(&spec.id).copied() {
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_SHOW);
                        SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                        )
                        .map_err(|error| error.to_string())?;
                        let _ = SetForegroundWindow(hwnd);
                    }
                    return Ok(());
                }
                let hwnd = unsafe { create_pin_window(app.clone(), spec.clone())? };
                windows.borrow_mut().insert(spec.id, hwnd);
                Ok(())
            });
            let _ = reply.send(result);
        }
        Command::Close { id, reply } => {
            let result = WINDOWS.with(|windows| {
                let hwnd = windows.borrow().get(&id).copied().ok_or("贴图窗口未打开")?;
                unsafe { DestroyWindow(hwnd).map_err(|error| error.to_string()) }
            });
            let _ = reply.send(result);
        }
        Command::PersistAll { reply } => {
            let result = WINDOWS.with(|windows| {
                for hwnd in windows.borrow().values() {
                    unsafe {
                        if let Some(data) = window_data(*hwnd) {
                            save_geometry(*hwnd, data)?;
                        }
                    }
                }
                Ok(())
            });
            let _ = reply.send(result);
        }
    }
}

unsafe fn create_pin_window(app: tauri::AppHandle, spec: PinWindowSpec) -> Result<HWND, String> {
    let image = image::open(&spec.image_path)
        .map_err(|error| format!("读取贴图失败: {error}"))?
        .into_rgba8();
    let initial_zoom = constrained_zoom(image.width(), image.height(), spec.zoom);
    let (source_dc, source_bitmap, source_previous, source_bits) = create_source_surface(&image)?;
    let data = Box::new(WindowData {
        app,
        id: spec.id.clone(),
        workspace_id: spec.workspace_id,
        image_id: spec.image_id,
        image_path: spec.image_path,
        image_width: image.width(),
        image_height: image.height(),
        source_dc,
        source_bitmap,
        source_previous,
        source_bits,
        zoom: initial_zoom,
    });
    let pointer = Box::into_raw(data);
    let title = wide(&format!("WW OCR 贴图 {}", spec.id));
    let width = scaled_dimension(spec.width, initial_zoom);
    let height = scaled_dimension(spec.height, initial_zoom);
    let (x, y) = spec
        .position
        .or_else(|| spec.saved.as_ref().map(|state| (state.x, state.y)))
        .unwrap_or_else(|| {
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);
            (cursor.x - width / 2, cursor.y - height / 2)
        });
    let module = GetModuleHandleW(None).map_err(|error| error.to_string())?;
    let result = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        w!("WWOcrNativePinWindow"),
        PCWSTR(title.as_ptr()),
        WS_POPUP,
        x,
        y,
        width,
        height,
        None,
        None,
        Some(HINSTANCE(module.0)),
        Some(pointer.cast()),
    );
    let hwnd = match result {
        Ok(hwnd) => hwnd,
        Err(error) => {
            return Err(format!("创建原生贴图窗口失败: {error}"));
        }
    };
    let data = window_data(hwnd).ok_or("原生贴图窗口状态初始化失败")?;
    if let Err(error) = render(hwnd, data, RenderQuality::High) {
        let _ = DestroyWindow(hwnd);
        return Err(error);
    }
    let _ = ShowWindow(hwnd, SW_SHOW);
    SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        x,
        y,
        width,
        height,
        SET_WINDOW_POS_FLAGS(0),
    )
    .map_err(|error| error.to_string())?;
    Ok(hwnd)
}

struct WindowData {
    app: tauri::AppHandle,
    id: String,
    workspace_id: String,
    image_id: String,
    image_path: PathBuf,
    image_width: u32,
    image_height: u32,
    source_dc: HDC,
    source_bitmap: HBITMAP,
    source_previous: HGDIOBJ,
    source_bits: *const u8,
    zoom: f64,
}

impl Drop for WindowData {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.source_dc, self.source_previous);
            let _ = DeleteObject(HGDIOBJ(self.source_bitmap.0));
            let _ = DeleteDC(self.source_dc);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return LRESULT(1);
    }

    let data = window_data(hwnd);
    match message {
        WM_LBUTTONDOWN => {
            let _ = ReleaseCapture();
            SendMessageW(
                hwnd,
                WM_NCLBUTTONDOWN,
                Some(WPARAM(HTCAPTION as usize)),
                Some(LPARAM(0)),
            );
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK | WM_NCLBUTTONDBLCLK => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            if let Some(data) = data {
                let delta = ((wparam.0 >> 16) as u16) as i16;
                let next = wheel_zoom(data.zoom, delta);
                if let Err(error) = set_zoom(hwnd, data, next, false) {
                    emit_error(&data.app, error);
                } else if SetTimer(Some(hwnd), ZOOM_SETTLE_TIMER, ZOOM_SETTLE_MS, None) == 0 {
                    emit_error(&data.app, "创建贴图缩放定时器失败".into());
                }
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == ZOOM_SETTLE_TIMER => {
            let _ = KillTimer(Some(hwnd), ZOOM_SETTLE_TIMER);
            if let Some(data) = data {
                let result = render(hwnd, data, RenderQuality::High)
                    .and_then(|_| persist_zoom(hwnd, data));
                if let Err(error) = result {
                    emit_error(&data.app, error);
                }
            }
            LRESULT(0)
        }
        WM_CONTEXTMENU => {
            if let Some(data) = data {
                show_context_menu(hwnd, data, lparam);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if let Some(data) = data {
                match wparam.0 as u16 {
                    0x1B => {
                        let _ = DestroyWindow(hwnd);
                    }
                    0x30 => {
                        if let Err(error) = set_zoom(hwnd, data, 1.0, true) {
                            emit_error(&data.app, error);
                        }
                    }
                    0x43 if GetKeyState(VK_CONTROL.0 as i32) < 0 => {
                        copy_image(data);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_EXITSIZEMOVE => {
            if let Some(data) = data {
                if let Err(error) = save_geometry(hwnd, data) {
                    emit_error(&data.app, error);
                }
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            if let Some(data) = data {
                let _ = KillTimer(Some(hwnd), ZOOM_SETTLE_TIMER);
                let _ = pins::repository::zoom(&data.app.state::<Database>(), &data.id, data.zoom);
                let _ = save_geometry(hwnd, data);
                let quitting = data
                    .app
                    .state::<QuitState>()
                    .0
                    .load(std::sync::atomic::Ordering::Acquire);
                if !quitting {
                    let _ =
                        pins::repository::set_open(&data.app.state::<Database>(), &data.id, false);
                    let _ = data.app.emit("desktop:changed", ());
                }
                WINDOWS.with(|windows| {
                    windows.borrow_mut().remove(&data.id);
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(data as *mut WindowData));
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn window_data(hwnd: HWND) -> Option<&'static mut WindowData> {
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowData;
    pointer.as_mut()
}

unsafe fn set_zoom(
    hwnd: HWND,
    data: &mut WindowData,
    value: f64,
    persist: bool,
) -> Result<(), String> {
    let next = constrained_zoom(data.image_width, data.image_height, value);
    if (next - data.zoom).abs() < f64::EPSILON {
        return Ok(());
    }
    let previous = data.zoom;
    data.zoom = next;
    let quality = if persist {
        RenderQuality::High
    } else {
        RenderQuality::Fast
    };
    if let Err(error) = render(hwnd, data, quality) {
        data.zoom = previous;
        return Err(error);
    }
    if persist {
        let _ = KillTimer(Some(hwnd), ZOOM_SETTLE_TIMER);
        persist_zoom(hwnd, data)?;
    }
    Ok(())
}

unsafe fn persist_zoom(hwnd: HWND, data: &WindowData) -> Result<(), String> {
    pins::repository::zoom(&data.app.state::<Database>(), &data.id, data.zoom)?;
    save_geometry(hwnd, data)?;
    data.app
        .emit("desktop:changed", ())
        .map_err(|error| error.to_string())
}

unsafe fn render(
    hwnd: HWND,
    data: &WindowData,
    quality: RenderQuality,
) -> Result<(), String> {
    let width = scaled_dimension(data.image_width, data.zoom) as u32;
    let height = scaled_dimension(data.image_height, data.zoom) as u32;

    if width == data.image_width && height == data.image_height {
        return update_layered_window(hwnd, data.source_dc, width, height, None);
    }

    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut destination = std::ptr::null_mut();
    let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut destination, None, 0)
        .map_err(|error| format!("创建贴图位图失败: {error}"))?;
    let byte_len = width as usize * height as usize * 4;
    let destination_pixels = std::slice::from_raw_parts_mut(destination.cast::<u8>(), byte_len);

    let dc = CreateCompatibleDC(None);
    if dc.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        return Err("创建贴图绘制上下文失败".into());
    }
    let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
    let result = (|| -> Result<(), String> {
        let source_len = data.image_width as usize * data.image_height as usize * 4;
        let source = std::slice::from_raw_parts(data.source_bits, source_len);
        let filter = match quality {
            RenderQuality::Fast => FilterType::Triangle,
            RenderQuality::High => FilterType::Lanczos3,
        };
        let resized = resize_premultiplied_bgra(
            source,
            data.image_width,
            data.image_height,
            width,
            height,
            filter,
        )?;
        destination_pixels.copy_from_slice(&resized);
        update_layered_window(hwnd, dc, width, height, None)
    })();
    SelectObject(dc, previous);
    let _ = DeleteDC(dc);
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    result
}

fn resize_premultiplied_bgra(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    filter: FilterType,
) -> Result<Vec<u8>, String> {
    let image = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(source_width, source_height, source)
        .ok_or("贴图源像素尺寸无效")?;
    Ok(image::imageops::resize(&image, width, height, filter).into_raw())
}

unsafe fn update_layered_window(
    hwnd: HWND,
    dc: HDC,
    width: u32,
    height: u32,
    destination: Option<&POINT>,
) -> Result<(), String> {
    let size = SIZE {
        cx: width as i32,
        cy: height as i32,
    };
    let origin = POINT { x: 0, y: 0 };
    let blend = blend_function();
    UpdateLayeredWindow(
        hwnd,
        None,
        destination.map(|point| point as *const POINT),
        Some(&size),
        Some(dc),
        Some(&origin),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    )
    .map_err(|error| format!("更新贴图窗口失败: {error}"))
}

unsafe fn create_source_surface(
    image: &RgbaImage,
) -> Result<(HDC, HBITMAP, HGDIOBJ, *const u8), String> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: image.width() as i32,
            biHeight: -(image.height() as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut destination = std::ptr::null_mut();
    let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut destination, None, 0)
        .map_err(|error| format!("创建贴图源位图失败: {error}"))?;
    let output = std::slice::from_raw_parts_mut(
        destination.cast::<u8>(),
        image.width() as usize * image.height() as usize * 4,
    );
    write_premultiplied_bgra(image.as_raw(), output);
    let dc = CreateCompatibleDC(None);
    if dc.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        return Err("创建贴图源绘制上下文失败".into());
    }
    let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
    Ok((dc, bitmap, previous, destination.cast::<u8>()))
}

fn blend_function() -> BLENDFUNCTION {
    BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    }
}

fn constrained_zoom(width: u32, height: u32, requested: f64) -> f64 {
    let requested = if requested.is_finite() { requested } else { 1.0 };
    if width == 0 || height == 0 {
        return requested.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    let dimension_limit = (MAX_RENDER_DIMENSION as f64 - 0.5) / width.max(height) as f64;
    let pixel_limit = (MAX_RENDER_PIXELS as f64 / (width as f64 * height as f64)).sqrt();
    let mut maximum = MAX_ZOOM.min(dimension_limit).min(pixel_limit);

    for _ in 0..4 {
        let rendered_width = (width as f64 * maximum).round().max(1.0) as u64;
        let rendered_height = (height as f64 * maximum).round().max(1.0) as u64;
        let rendered_pixels = rendered_width.saturating_mul(rendered_height);
        if rendered_pixels <= MAX_RENDER_PIXELS {
            break;
        }
        maximum *= (MAX_RENDER_PIXELS as f64 / rendered_pixels as f64).sqrt() * 0.999_999;
    }

    maximum = maximum.max(f64::MIN_POSITIVE);
    requested.clamp(MIN_ZOOM.min(maximum), maximum)
}

fn wheel_zoom(current: f64, delta: i16) -> f64 {
    (current * 1.1_f64.powf(delta as f64 / 120.0)).clamp(MIN_ZOOM, MAX_ZOOM)
}
unsafe fn show_context_menu(hwnd: HWND, data: &mut WindowData, lparam: LPARAM) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let _ = AppendMenuW(menu, MF_STRING, MENU_COPY as usize, w!("复制图片"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_OCR as usize, w!("OCR 并复制文字"));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, MENU_RESET as usize, w!("恢复原始比例"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_CLOSE as usize, w!("关闭贴图"));

    let mut point = if lparam.0 == -1 {
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        cursor
    } else {
        POINT {
            x: lparam.0 as i16 as i32,
            y: (lparam.0 >> 16) as i16 as i32,
        }
    };
    if point.x == -1 && point.y == -1 {
        let _ = GetCursorPos(&mut point);
    }
    let _ = SetForegroundWindow(hwnd);
    let choice = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        None,
        hwnd,
        None,
    )
    .0;
    let _ = DestroyMenu(menu);

    match choice {
        MENU_COPY => copy_image(data),
        MENU_OCR => recognize(data),
        MENU_RESET => {
            if let Err(error) = set_zoom(hwnd, data, 1.0, true) {
                emit_error(&data.app, error);
            }
        }
        MENU_CLOSE => {
            let _ = DestroyWindow(hwnd);
        }
        _ => {}
    }
}

fn copy_image(data: &WindowData) {
    let app = data.app.clone();
    let path = data.image_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<(), String> {
            let image = image::open(path)
                .map_err(|error| error.to_string())?
                .into_rgba8();
            let (width, height) = image.dimensions();
            app.clipboard()
                .write_image(&tauri::image::Image::new_owned(
                    image.into_raw(),
                    width,
                    height,
                ))
                .map_err(|error| error.to_string())
        })();
        if let Err(error) = result {
            emit_error(&app, error);
        }
    });
}

fn recognize(data: &WindowData) {
    let app = data.app.clone();
    let image_id = data.image_id.clone();
    let workspace_id = data.workspace_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ocr_actions::recognize_asset(&app, &image_id).and_then(|result| {
            app.clipboard()
                .write_text(result.text)
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(()) => {
                let _ = app.emit("ocr:changed", workspace_id);
            }
            Err(error) => emit_error(&app, error),
        }
    });
}

unsafe fn save_geometry(hwnd: HWND, data: &WindowData) -> Result<(), String> {
    let mut rect = RECT::default();
    GetWindowRect(hwnd, &mut rect).map_err(|error| error.to_string())?;
    repository::save(
        &data.app.state::<Database>(),
        "pin",
        &data.id,
        &WindowState {
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left).max(1) as u32,
            height: (rect.bottom - rect.top).max(1) as u32,
            topmost: true,
        },
    )
}

fn emit_error(app: &tauri::AppHandle, error: String) {
    log::error!("native_pin_failed reason={error}");
    let _ = app.emit_to("main", "desktop:error", error);
}

fn scaled_dimension(value: u32, zoom: f64) -> i32 {
    ((value as f64 * zoom.clamp(f64::MIN_POSITIVE, MAX_ZOOM)).round() as i64)
        .clamp(1, MAX_RENDER_DIMENSION as i64) as i32
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn write_premultiplied_bgra(source: &[u8], output: &mut [u8]) {
    for (source, target) in source.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
        let alpha = source[3] as u16;
        target[0] = ((source[2] as u16 * alpha + 127) / 255) as u8;
        target[1] = ((source[1] as u16 * alpha + 127) / 255) as u8;
        target[2] = ((source[0] as u16 * alpha + 127) / 255) as u8;
        target[3] = source[3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_pin_dimensions_with_bounds() {
        assert_eq!(scaled_dimension(3840, 0.1), 384);
        assert_eq!(scaled_dimension(100, 1.25), 125);
        assert_eq!(scaled_dimension(0, 1.0), 1);
    }

    #[test]
    fn constrains_zoom_by_multiplier_dimension_and_pixels() {
        assert_eq!(constrained_zoom(100, 100, 9.0), MAX_ZOOM);
        assert_eq!(constrained_zoom(100, 100, f64::NAN), 1.0);

        let zoom = constrained_zoom(3840, 2160, 5.0);
        let width = (3840.0 * zoom).round() as u64;
        let height = (2160.0 * zoom).round() as u64;
        assert!(width <= MAX_RENDER_DIMENSION as u64);
        assert!(height <= MAX_RENDER_DIMENSION as u64);
        assert!(width * height <= MAX_RENDER_PIXELS);
    }

    #[test]
    fn supports_partial_wheel_deltas_and_zoom_bounds() {
        assert!(wheel_zoom(1.0, 15) > 1.0);
        assert_eq!(wheel_zoom(MAX_ZOOM, 120), MAX_ZOOM);
        assert_eq!(wheel_zoom(MIN_ZOOM, -120), MIN_ZOOM);
    }

    #[test]
    fn resampling_preserves_opaque_alpha_and_dimensions() {
        let source = [
            0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255,
        ];
        for filter in [FilterType::Triangle, FilterType::Lanczos3] {
            let scaled = resize_premultiplied_bgra(&source, 2, 2, 4, 4, filter).unwrap();
            assert_eq!(scaled.len(), 4 * 4 * 4);
            assert!(scaled.chunks_exact(4).all(|pixel| pixel[3] == 255));
        }
        assert!(
            resize_premultiplied_bgra(&source[..3], 2, 2, 4, 4, FilterType::Lanczos3)
                .is_err()
        );
    }
    #[test]
    fn converts_rgba_to_premultiplied_bgra() {
        let source = [200, 100, 50, 128, 1, 2, 3, 0];
        let mut output = [0; 8];
        write_premultiplied_bgra(&source, &mut output);
        assert_eq!(output, [25, 50, 100, 128, 0, 0, 0, 0]);
    }
}
