//! Window and control detection / interpretation module.
//!
//! Provides helpers that inspect a target window and its child controls to
//! determine which automation strategy is most likely to succeed.  The heuristic
//! favours UI Automation (richer API, pattern support) but falls back to the
//! Win32 strategy when it detects Delphi/VCL-specific window classes or when
//! UI Automation fails to enumerate any children.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetWindowTextW};

/// Known Delphi/VCL window class prefixes that often cause issues with
/// UI Automation and respond better to raw Win32 messages.
const DELPHI_CLASS_PREFIXES: &[&str] = &[
    "TForm",
    "TEdit",
    "TButton",
    "TLabel",
    "TComboBox",
    "TCheckBox",
    "TRadioButton",
    "TListBox",
    "TMemo",
    "TPanel",
    "TGroupBox",
    "TPageControl",
    "TTabSheet",
    "TStringGrid",
    "TDateTimePicker",
    "TStatusBar",
    "TToolBar",
    "TTreeView",
    "TListView",
    "TRichEdit",
    "TSpinEdit",
];

/// Summary of what was detected about a window.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub class_name: String,
    pub title: String,
    /// `true` when the window class looks like a Delphi/VCL control.
    pub is_delphi: bool,
}

/// Inspect a window handle and return [`WindowInfo`].
pub fn detect_window(hwnd: HWND, log: &mut dyn FnMut(&str)) -> WindowInfo {
    let class_name = get_class_name(hwnd);
    let title = get_window_title(hwnd);
    let is_delphi = DELPHI_CLASS_PREFIXES.iter().any(|p| class_name.starts_with(p));

    log(&format!(
        "[detection] hwnd={:?} class={:?} title={:?} delphi={}",
        hwnd, class_name, title, is_delphi
    ));

    WindowInfo {
        hwnd,
        class_name,
        title,
        is_delphi,
    }
}

/// Suggest strategy ordering based on the detected window.
///
/// Returns a list of strategy names in preferred order.  The caller should try
/// each strategy in order until one succeeds.
pub fn recommend_strategy_order(info: &WindowInfo, log: &mut dyn FnMut(&str)) -> Vec<String> {
    let order = if info.is_delphi {
        // Delphi apps: try Win32 first (more reliable for VCL), then UIA.
        log("[detection] Delphi window detected — preferring Win32 strategy");
        vec!["Win32".to_string(), "UIAutomation".to_string()]
    } else {
        // Non-Delphi apps: UIA is generally better.
        log("[detection] Non-Delphi window — preferring UIAutomation strategy");
        vec!["UIAutomation".to_string(), "Win32".to_string()]
    };
    order
}

// ─── Win32 helpers ───────────────────────────────────────────────────────────

fn get_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..len])
}

fn get_window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..len])
}
