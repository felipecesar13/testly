//! Legacy inspector module — thin wrapper around `automation::fallback`.
//!
//! Kept for backward compatibility.  The `run_direct` code path in `main.rs`
//! still calls these functions.  Internally they delegate to the new
//! multi-strategy [`AutomationManager`](crate::automation::fallback::AutomationManager).

#![allow(dead_code)]

use windows::Win32::Foundation::HWND;

use crate::automation::fallback::AutomationManager;

pub fn list_components(hwnd: HWND, log: &mut dyn FnMut(&str)) -> Result<(), Box<dyn std::error::Error>> {
    let manager = AutomationManager::with_defaults();
    manager.list_components(hwnd, log).map_err(|e| -> Box<dyn std::error::Error> { e })
}

pub fn find_and_click(
    hwnd: HWND,
    control_type_str: &str,
    name: &str,
    log: &mut dyn FnMut(&str),
) -> Result<(), Box<dyn std::error::Error>> {
    let manager = AutomationManager::with_defaults();
    manager.find_and_click(hwnd, control_type_str, name, log).map_err(|e| -> Box<dyn std::error::Error> { e })
}

pub fn find_and_fill(
    hwnd: HWND,
    name: &str,
    value: &str,
    log: &mut dyn FnMut(&str),
) -> Result<(), Box<dyn std::error::Error>> {
    let manager = AutomationManager::with_defaults();
    manager.find_and_fill(hwnd, name, value, log).map_err(|e| -> Box<dyn std::error::Error> { e })
}
