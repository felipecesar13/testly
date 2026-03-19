use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use windows::Win32::Foundation::HWND;

use crate::automation::fallback::AutomationManager;
use crate::launcher;
use crate::parser::{Command, TestFile};

#[derive(Debug)]
pub struct TestResult {
    pub file: String,
    pub description: Option<String>,
    pub passed: bool,
    pub error: Option<String>,
    pub duration_ms: u128,
    pub log: Vec<String>,
}

pub fn run_all(test_files: Vec<TestFile>) -> Vec<TestResult> {
    let results: Arc<Mutex<Vec<TestResult>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for test_file in test_files {
        let results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            let result = run_single(test_file);
            results.lock().unwrap().push(result);
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    Arc::try_unwrap(results).unwrap().into_inner().unwrap()
}

fn run_single(test_file: TestFile) -> TestResult {
    let start = Instant::now();
    let verbose = test_file.is_verbose();
    let description = test_file.description();
    let file_path = test_file.path.clone();
    let mut log: Vec<String> = Vec::new();

    let mut log_fn = |msg: &str| {
        if verbose {
            let _ = writeln!(std::io::stdout(), "[{}] {}", file_path, msg);
        }
        log.push(msg.to_string());
    };

    log_fn(&format!("=== Starting test: {} ===", file_path));
    if let Some(ref desc) = description {
        log_fn(&format!("Description: {}", desc));
    }

    // Create the fallback-aware automation manager with all built-in strategies.
    let manager = AutomationManager::with_defaults();
    log_fn(&format!(
        "[automation] Available strategies: {:?}",
        manager.strategy_names()
    ));

    let mut current_hwnd: Option<HWND> = None;
    let mut _child_process: Option<std::process::Child> = None;

    for cmd in &test_file.commands {
        match cmd {
            Command::Verbose | Command::Describe(_) => {
                // Already handled above
            }

            Command::Launch(path) => {
                log_fn(&format!("> launch(\"{}\")", path));
                match launcher::launch_and_find_window_with_log(path, &mut log_fn) {
                    Ok((child, hwnd)) => {
                        current_hwnd = Some(hwnd);
                        _child_process = Some(child);
                        log_fn("[OK] Application launched successfully");
                    }
                    Err(e) => {
                        let msg = format!("[FAIL] Failed to launch: {}", e);
                        log_fn(&msg);
                        return TestResult {
                            file: file_path,
                            description,
                            passed: false,
                            error: Some(msg),
                            duration_ms: start.elapsed().as_millis(),
                            log,
                        };
                    }
                }
            }

            Command::Inspect => {
                log_fn("> inspect()");
                match current_hwnd {
                    Some(hwnd) => {
                        if let Err(e) = manager.list_components(hwnd, &mut log_fn) {
                            let msg = format!("[FAIL] Inspect failed: {}", e);
                            log_fn(&msg);
                            return TestResult {
                                file: file_path,
                                description,
                                passed: false,
                                error: Some(msg),
                                duration_ms: start.elapsed().as_millis(),
                                log,
                            };
                        }
                        log_fn("[OK] Inspect completed");
                    }
                    None => {
                        let msg = "[FAIL] No application launched. Call launch() before inspect()".to_string();
                        log_fn(&msg);
                        return TestResult {
                            file: file_path,
                            description,
                            passed: false,
                            error: Some(msg),
                            duration_ms: start.elapsed().as_millis(),
                            log,
                        };
                    }
                }
            }

            Command::Click(control_type, name) => {
                log_fn(&format!("> click(\"{}\", \"{}\")", control_type, name));
                match current_hwnd {
                    Some(hwnd) => {
                        // Small delay to let UI settle
                        std::thread::sleep(std::time::Duration::from_millis(300));

                        if let Err(e) = manager.find_and_click(hwnd, control_type, name, &mut log_fn) {
                            let msg = format!("[FAIL] Click failed: {}", e);
                            log_fn(&msg);
                            return TestResult {
                                file: file_path,
                                description,
                                passed: false,
                                error: Some(msg),
                                duration_ms: start.elapsed().as_millis(),
                                log,
                            };
                        }
                        log_fn("[OK] Click completed");
                    }
                    None => {
                        let msg = "[FAIL] No application launched. Call launch() before click()".to_string();
                        log_fn(&msg);
                        return TestResult {
                            file: file_path,
                            description,
                            passed: false,
                            error: Some(msg),
                            duration_ms: start.elapsed().as_millis(),
                            log,
                        };
                    }
                }
            }

            Command::Fill(name, value) => {
                log_fn(&format!("> fill(\"{}\", \"{}\")", name, value));
                match current_hwnd {
                    Some(hwnd) => {
                        std::thread::sleep(std::time::Duration::from_millis(300));

                        if let Err(e) = manager.find_and_fill(hwnd, name, value, &mut log_fn) {
                            let msg = format!("[FAIL] Fill failed: {}", e);
                            log_fn(&msg);
                            return TestResult {
                                file: file_path,
                                description,
                                passed: false,
                                error: Some(msg),
                                duration_ms: start.elapsed().as_millis(),
                                log,
                            };
                        }
                        log_fn("[OK] Fill completed");
                    }
                    None => {
                        let msg = "[FAIL] No application launched. Call launch() before fill()".to_string();
                        log_fn(&msg);
                        return TestResult {
                            file: file_path,
                            description,
                            passed: false,
                            error: Some(msg),
                            duration_ms: start.elapsed().as_millis(),
                            log,
                        };
                    }
                }
            }

            Command::Wait(ms) => {
                log_fn(&format!("> wait({})", ms));
                std::thread::sleep(std::time::Duration::from_millis(*ms));
                log_fn(&format!("[OK] Waited {}ms", ms));
            }
        }
    }

    log_fn(&format!("=== Test completed in {}ms ===", start.elapsed().as_millis()));

    TestResult {
        file: file_path,
        description,
        passed: true,
        error: None,
        duration_ms: start.elapsed().as_millis(),
        log,
    }
}
