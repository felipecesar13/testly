use std::collections::HashSet;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
};

pub fn launch_and_find_window(path: &str) -> Result<(Child, HWND), Box<dyn std::error::Error>> {
    launch_and_find_window_with_log(path, &mut |msg| println!("{}", msg))
}

pub fn launch_and_find_window_with_log(
    path: &str,
    log: &mut dyn FnMut(&str),
) -> Result<(Child, HWND), Box<dyn std::error::Error>> {
    // Snapshot existing visible windows before launch
    let before = collect_visible_windows();

    let child = Command::new(path).spawn()?;
    let pid = child.id();

    for attempt in 1..=10 {
        thread::sleep(Duration::from_millis(500));

        // Strategy 1: Match by PID
        if let Some(hwnd) = find_window_by_pid(pid) {
            log(&format!("[testly] Window found (by PID) after {}ms", attempt * 500));
            return Ok((child, hwnd));
        }

        // Strategy 2: Detect new windows that appeared after launch
        let after = collect_visible_windows();
        let new_windows: Vec<isize> = after.difference(&before).copied().collect();
        if let Some(&hwnd_val) = new_windows.first() {
            log(&format!("[testly] Window found (new window) after {}ms", attempt * 500));
            return Ok((child, HWND(hwnd_val as *mut _)));
        }
    }

    Err(format!(
        "Could not find a visible window for PID {} after 5 seconds",
        pid
    )
    .into())
}

fn collect_visible_windows() -> HashSet<isize> {
    let mut windows: Vec<isize> = Vec::new();

    unsafe {
        let _ = EnumWindows(
            Some(collect_callback),
            LPARAM(&mut windows as *mut Vec<isize> as isize),
        );
    }

    windows.into_iter().collect()
}

unsafe extern "system" fn collect_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        if IsWindowVisible(hwnd).as_bool() {
            let vec = &mut *(lparam.0 as *mut Vec<isize>);
            vec.push(hwnd.0 as isize);
        }
        BOOL(1)
    }
}

struct FindByPid {
    target_pid: u32,
    found_hwnd: Option<HWND>,
}

fn find_window_by_pid(pid: u32) -> Option<HWND> {
    let mut data = FindByPid {
        target_pid: pid,
        found_hwnd: None,
    };

    unsafe {
        let _ = EnumWindows(
            Some(find_by_pid_callback),
            LPARAM(&mut data as *mut FindByPid as isize),
        );
    }

    data.found_hwnd
}

unsafe extern "system" fn find_by_pid_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let data = &mut *(lparam.0 as *mut FindByPid);
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid == data.target_pid && IsWindowVisible(hwnd).as_bool() {
            data.found_hwnd = Some(hwnd);
            return BOOL(0);
        }

        BOOL(1)
    }
}
