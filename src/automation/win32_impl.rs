//! Pure Win32–based strategy.
//!
//! Uses `SendMessageW`, `EnumChildWindows`, `GetClassName`, and related Win32
//! APIs to interact with window controls.  This strategy does **not** depend on
//! the UI Automation COM infrastructure and is therefore more reliable for
//! applications whose accessibility providers are incomplete (e.g. some Delphi
//! / VCL builds).
//!
//! # Limitations
//!
//! * Control discovery relies on window-class names and window text; it cannot
//!   query rich control metadata the way UI Automation can.
//! * Some modern UI frameworks draw their own controls inside a single HWND,
//!   making them invisible to `EnumChildWindows`.

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, GetWindowTextW, SendMessageW,
    BM_CLICK, WM_SETTEXT,
};

use super::{AutomationStrategy, StrategyResult};

/// Strategy backed by raw Win32 window messages.
pub struct Win32Strategy;

impl Win32Strategy {
    pub fn new() -> Self {
        Self
    }
}

impl AutomationStrategy for Win32Strategy {
    fn name(&self) -> &str {
        "Win32"
    }

    fn list_components(
        &self,
        hwnd: HWND,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        log(&format!(
            "{:<6} {:<20} {:<30} {}",
            "#", "ClassName", "Text", "HWND"
        ));
        log(&"-".repeat(80));

        let mut ctx = EnumCtx {
            counter: 1,
            entries: Vec::new(),
        };

        unsafe {
            let _ = EnumChildWindows(
                hwnd,
                Some(enum_children_cb),
                LPARAM(&mut ctx as *mut EnumCtx as isize),
            );
        }

        for entry in &ctx.entries {
            log(&format!(
                "{:<6} {:<20} {:<30} {:?}",
                entry.index, entry.class_name, truncate(&entry.text, 28), entry.hwnd
            ));
        }

        log(&format!(
            "\n[testly] Total: {} components found.",
            ctx.entries.len()
        ));
        Ok(())
    }

    fn find_and_click(
        &self,
        hwnd: HWND,
        control_type: &str,
        name: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        // Map friendly type names to common Win32 class-name patterns.
        let class_pattern = friendly_to_class(control_type);

        let children = enum_children(hwnd);

        // Find matching child: class contains the pattern AND text matches.
        let target = children.iter().find(|c| {
            class_matches(&c.class_name, &class_pattern)
                && text_matches(&c.text, name)
        });

        match target {
            Some(entry) => {
                log(&format!(
                    "[testly] Found element: {} \"{}\" (class={}, hwnd={:?})",
                    control_type, name, entry.class_name, entry.hwnd
                ));

                // Send BM_CLICK for button-like controls; otherwise fall back to
                // generic WM_COMMAND-style click.
                unsafe {
                    SendMessageW(entry.hwnd, BM_CLICK, WPARAM(0), LPARAM(0));
                }

                log(&format!(
                    "[testly] Clicked: {} \"{}\" (BM_CLICK)",
                    control_type, name
                ));
                Ok(())
            }
            None => Err(format!(
                "Element not found via Win32: {} \"{}\"",
                control_type, name
            )
            .into()),
        }
    }

    fn find_and_fill(
        &self,
        hwnd: HWND,
        field_name: &str,
        value: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        let children = enum_children(hwnd);

        // Strategy A: find an edit control whose window text matches `field_name`.
        // Strategy B: find a label that matches `field_name`, then pick the next
        //             sibling edit control (common Delphi pattern: TLabel + TEdit).
        let target = find_edit_by_name(&children, field_name)
            .or_else(|| find_edit_after_label(&children, field_name));

        match target {
            Some(target_hwnd) => {
                log(&format!(
                    "[testly] Found input: \"{}\" (hwnd={:?})",
                    field_name, target_hwnd
                ));

                let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();

                unsafe {
                    SendMessageW(
                        target_hwnd,
                        WM_SETTEXT,
                        WPARAM(0),
                        LPARAM(wide.as_ptr() as isize),
                    );
                }

                log(&format!(
                    "[testly] Filled: \"{}\" = \"{}\" (WM_SETTEXT)",
                    field_name, value
                ));
                Ok(())
            }
            None => Err(format!(
                "Edit element not found via Win32: \"{}\"",
                field_name
            )
            .into()),
        }
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Describes one child window discovered by `EnumChildWindows`.
struct ChildEntry {
    index: u32,
    hwnd: HWND,
    class_name: String,
    text: String,
}

struct EnumCtx {
    counter: u32,
    entries: Vec<ChildEntry>,
}

unsafe extern "system" fn enum_children_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let ctx = &mut *(lparam.0 as *mut EnumCtx);

        let class_name = get_class_name(hwnd);
        let text = get_window_text(hwnd);

        ctx.entries.push(ChildEntry {
            index: ctx.counter,
            hwnd,
            class_name,
            text,
        });
        ctx.counter += 1;

        BOOL(1) // continue enumeration
    }
}

fn enum_children(hwnd: HWND) -> Vec<ChildEntry> {
    let mut ctx = EnumCtx {
        counter: 1,
        entries: Vec::new(),
    };

    unsafe {
        let _ = EnumChildWindows(
            hwnd,
            Some(enum_children_cb),
            LPARAM(&mut ctx as *mut EnumCtx as isize),
        );
    }

    ctx.entries
}

fn get_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..len])
}

fn get_window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) } as usize;
    String::from_utf16_lossy(&buf[..len])
}

/// Map friendly control-type names (from the testly DSL) to Win32 class-name
/// substrings.  Delphi classes start with `T` (e.g. `TButton`), standard
/// Windows classes use `Button`, `Edit`, etc.
fn friendly_to_class(friendly: &str) -> String {
    match friendly {
        "Button" => "Button".to_string(),
        "Edit" => "Edit".to_string(),
        "CheckBox" => "Button".to_string(),   // CheckBox is a Button style
        "RadioButton" => "Button".to_string(),
        "ComboBox" => "ComboBox".to_string(),
        "ListBox" => "ListBox".to_string(),
        "Text" | "Label" => "Static".to_string(),
        _ => friendly.to_string(), // pass-through for Delphi class names
    }
}

/// Case-insensitive class name matching that also handles the `T`-prefix
/// convention used by Delphi/VCL (e.g. `TButton` matches `Button` pattern).
fn class_matches(class: &str, pattern: &str) -> bool {
    let cl = class.to_ascii_lowercase();
    let pat = pattern.to_ascii_lowercase();
    cl.contains(&pat) || cl == format!("t{}", pat)
}

/// Check if window text matches the desired name (case-insensitive, trimmed).
fn text_matches(text: &str, name: &str) -> bool {
    text.trim().eq_ignore_ascii_case(name.trim())
}

/// Find an edit-like child whose text matches `field_name`.
fn find_edit_by_name(children: &[ChildEntry], field_name: &str) -> Option<HWND> {
    children
        .iter()
        .find(|c| is_edit_class(&c.class_name) && text_matches(&c.text, field_name))
        .map(|c| c.hwnd)
}

/// Find a label matching `field_name` and return the first edit-like sibling
/// that appears after it in the enumeration order.
fn find_edit_after_label(children: &[ChildEntry], field_name: &str) -> Option<HWND> {
    let label_idx = children.iter().position(|c| {
        is_label_class(&c.class_name) && text_matches(&c.text, field_name)
    })?;

    // Return the first edit-class child that comes after the label.
    children.get(label_idx + 1..)?
        .iter()
        .find(|c| is_edit_class(&c.class_name))
        .map(|c| c.hwnd)
}

fn is_edit_class(class: &str) -> bool {
    let cl = class.to_ascii_lowercase();
    cl.contains("edit") || cl.contains("memo") || cl.contains("richedit")
}

fn is_label_class(class: &str) -> bool {
    let cl = class.to_ascii_lowercase();
    cl.contains("static") || cl.contains("label") || cl.contains("tlabel")
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
