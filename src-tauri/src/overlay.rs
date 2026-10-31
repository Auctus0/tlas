//! Şampiyon seçimi kaplaması.
//!
//! Ayrı, çerçevesiz ve odak çalmayan bir pencere LoL istemcisinin yanına
//! hizalanır. Oyun içine injection yapılmaz; yalnızca LCU verisi gösterilir.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, IsIconic};
use windows::core::PCWSTR;

const WINDOW_LABEL: &str = "overlay";
const GAP: i32 = 8;
const EDGE: i32 = 24;

/// Kaplamanın gerçekten görünür olduğu tek doğruluk kaynağı. Faz izleyicisi ve
/// ana pencere aynı komutu çağırdığı için mükerrer show/emit yapılmaz.
static OVERLAY_SHOWN: AtomicBool = AtomicBool::new(false);

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn rect_of(hwnd: HWND) -> Option<RECT> {
    if hwnd.is_invalid() {
        return None;
    }

    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;

    (rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
}

fn find_client_hwnd() -> Option<HWND> {
    let class = wide("RiotWindowClass");
    if let Some(hwnd) = unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }
        .ok()
        .filter(|hwnd| !hwnd.is_invalid())
    {
        return Some(hwnd);
    }

    let title = wide("League of Legends");
    unsafe { FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) }
        .ok()
        .filter(|hwnd| !hwnd.is_invalid())
}

fn is_minimized(hwnd: HWND) -> bool {
    unsafe { IsIconic(hwnd) }.as_bool()
}

/// İstemcinin bulunduğu monitörün görev çubuğu hariç çalışma alanı.
fn monitor_work_area(hwnd: HWND) -> Option<RECT> {
    if hwnd.is_invalid() {
        return None;
    }

    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_invalid() {
        return None;
    }

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        (info.rcWork.right > info.rcWork.left && info.rcWork.bottom > info.rcWork.top)
            .then_some(info.rcWork)
    } else {
        None
    }
}

fn monitor_rect(app: &AppHandle) -> Option<RECT> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let origin = monitor.position();
    let size = monitor.size();

    Some(RECT {
        left: origin.x,
        top: origin.y,
        right: origin.x + size.width as i32,
        bottom: origin.y + size.height as i32,
    })
}

/// İstemci penceresine göre kaplama konumunu seçer.
///
/// Önce istemcinin solunu dener; orada yer yoksa sağına geçer. İkisi de
/// çalışma alanına sığmıyorsa (maximize istemci) sol kenara yapışır. Sonuç her
/// durumda çalışma alanının içine kırpılır; negatif koordinatlı monitörlerde
/// de doğru çalışır.
fn choose_position(client: RECT, work: RECT, width: i32, height: i32) -> (i32, i32) {
    let left = client.left - width - GAP;
    let right = client.right + GAP;

    let x = if left >= work.left {
        left
    } else if right + width <= work.right {
        right
    } else {
        work.left
    };

    let max_x = (work.right - width).max(work.left);
    let max_y = (work.bottom - height).max(work.top);

    (x.clamp(work.left, max_x), client.top.clamp(work.top, max_y))
}

/// İstemci minimized ise kaplamayı çalışma alanının sağına, dikeyde ortaya koy.
fn dock_right(work: RECT, width: i32, height: i32) -> (i32, i32) {
    let x = (work.right - width - EDGE).max(work.left);
    let y = work.top + ((work.bottom - work.top - height) / 2).max(0);
    (x, y)
}

fn screen_fallback(app: &AppHandle, width: i32, height: i32) -> PhysicalPosition<i32> {
    if let Some(work) = monitor_rect(app) {
        let (x, y) = dock_right(work, width, height);
        return PhysicalPosition::new(x, y);
    }

    PhysicalPosition::new(EDGE, 80)
}

fn target_position(app: &AppHandle, width: i32, height: i32) -> PhysicalPosition<i32> {
    let hwnd = find_client_hwnd();
    let work = hwnd
        .and_then(monitor_work_area)
        .or_else(|| monitor_rect(app));

    let client = match hwnd {
        Some(hwnd) if !is_minimized(hwnd) => rect_of(hwnd),
        _ => None,
    };

    match (client, work) {
        (Some(client), Some(work)) => {
            let (x, y) = choose_position(client, work, width, height);
            PhysicalPosition::new(x, y)
        }
        (None, Some(work)) => {
            let (x, y) = dock_right(work, width, height);
            PhysicalPosition::new(x, y)
        }
        _ => screen_fallback(app, width, height),
    }
}

fn overlay_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| "Kaplama penceresi bulunamadı.".to_string())
}

fn place(app: &AppHandle, window: &tauri::WebviewWindow) -> Result<(), String> {
    let size = window
        .outer_size()
        .map_err(|error| format!("Kaplama boyutu okunamadı: {error}"))?;

    let position = target_position(app, size.width as i32, size.height as i32);
    window
        .set_position(position)
        .map_err(|error| format!("Kaplama taşınamadı: {error}"))
}

/// Kaplama görünürlüğünü idempotent biçimde ayarlar.
pub fn set_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    let window = overlay_window(app)?;

    if !visible {
        if OVERLAY_SHOWN.swap(false, Ordering::SeqCst) {
            window
                .hide()
                .map_err(|error| format!("Kaplama gizlenemedi: {error}"))?;
        }
        return Ok(());
    }

    // Görünürken bile istemci taşınmış olabilir; konumu her seferinde tazele.
    place(app, &window)?;

    if OVERLAY_SHOWN.load(Ordering::SeqCst) {
        return Ok(());
    }

    window
        .show()
        .map_err(|error| format!("Kaplama gösterilemedi: {error}"))?;
    window
        .set_always_on_top(true)
        .map_err(|error| format!("Kaplama öne alınamadı: {error}"))?;
    OVERLAY_SHOWN.store(true, Ordering::SeqCst);

    let _ = window.emit("tlas:overlay-shown", ());
    Ok(())
}

#[tauri::command]
pub fn set_overlay_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    set_visible(&app, visible)
}

#[tauri::command]
pub fn place_overlay(app: AppHandle) -> Result<(), String> {
    let window = overlay_window(&app)?;
    place(&app, &window)
}

/// Arka planda oyun fazını izler; ana pencere tepsiye kapalıyken de kaplamayı
/// şampiyon seçimi ve aktif maç boyunca açar/kapatır.
pub fn spawn_phase_watch(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_phase: Option<Option<String>> = None;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let (phase, overlay_enabled) = {
                use tauri::Manager;
                let state = app.state::<crate::state::AppState>();
                (
                    state.lcu.phase().await.ok(),
                    state.config.get().overlay_enabled,
                )
            };

            let should_show = overlay_enabled
                && matches!(
                    phase.as_deref(),
                    Some("ChampSelect" | "GameStart" | "InProgress" | "Reconnect")
                );
            // Görünürken her turda konum tazelenir; istemci taşınırsa takip eder.
            let _ = set_visible(&app, should_show);

            if last_phase.as_ref() != Some(&phase) {
                last_phase = Some(phase.clone());
                let _ = app.emit("tlas:game-phase", phase);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{choose_position, dock_right};

    fn is_within(pos: (i32, i32), work: windows::Win32::Foundation::RECT, w: i32, h: i32) -> bool {
        pos.0 >= work.left
            && pos.0 + w <= work.right
            && pos.1 >= work.top
            && pos.1 + h <= work.bottom
    }

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> windows::Win32::Foundation::RECT {
        windows::Win32::Foundation::RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn maximizes_to_left_edge_when_both_sides_are_full() {
        let work = rect(0, 0, 1920, 1040);
        let client = rect(0, 0, 1920, 1040);

        let pos = choose_position(client, work, 360, 400);

        assert_eq!(pos, (0, 0));
        assert!(is_within(pos, work, 360, 400));
    }

    #[test]
    fn uses_left_side_when_it_fits() {
        let work = rect(0, 0, 1920, 1040);
        let client = rect(500, 100, 1000, 800);

        assert_eq!(choose_position(client, work, 360, 400), (132, 100));
    }

    #[test]
    fn falls_back_to_right_side_when_left_is_off_screen() {
        let work = rect(0, 0, 2560, 1400);
        let client = rect(80, 100, 1200, 900);

        assert_eq!(choose_position(client, work, 360, 400), (1208, 100));
    }

    #[test]
    fn clamps_inside_negative_monitor() {
        let work = rect(-1920, 0, 0, 1040);
        let client = rect(-1100, 80, -300, 800);

        let pos = choose_position(client, work, 360, 400);

        assert_eq!(pos, (-1468, 80));
        assert!(is_within(pos, work, 360, 400));
    }

    #[test]
    fn clamps_vertical_position_into_work_area() {
        let work = rect(0, 0, 1920, 1040);
        let client = rect(500, -300, 1200, 500);

        assert_eq!(choose_position(client, work, 360, 400).1, 0);

        let low = rect(500, 900, 1200, 1300);
        assert_eq!(choose_position(low, work, 360, 400).1, 640);
    }

    #[test]
    fn docks_right_for_minimized_client() {
        let work = rect(0, 0, 1920, 1040);

        assert_eq!(dock_right(work, 360, 400), (1536, 320));
        assert!(is_within(dock_right(work, 360, 400), work, 360, 400));
    }
}
