#![allow(non_upper_case_globals)]

mod automation;
mod inspector;
mod launcher;
mod parser;
mod runner;

use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Testly - Desktop Application Automated Testing Tool");
        println!();
        println!("Usage:");
        println!("  testly <file.testly> [file2.testly ...]   Run test files (parallel)");
        println!("  testly <folder>                           Run all .testly files in folder");
        println!("  testly <application_path>                 Launch and inspect an app");
        return Ok(());
    }

    // Collect .testly files from args (expand directories)
    let mut testly_paths: Vec<String> = Vec::new();

    for arg in &args[1..] {
        let path = Path::new(arg);
        if path.is_dir() {
            match fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.extension().and_then(|e| e.to_str()) == Some("testly") {
                            testly_paths.push(p.display().to_string());
                        }
                    }
                }
                Err(e) => eprintln!("[testly] Cannot read directory {}: {}", arg, e),
            }
        } else if arg.ends_with(".testly") {
            testly_paths.push(arg.clone());
        }
    }

    if !testly_paths.is_empty() {
        let refs: Vec<&String> = testly_paths.iter().collect();
        run_test_files(&refs)
    } else {
        run_direct(&args[1])
    }
}

fn run_test_files(paths: &[&String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut test_files = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);
        if !path.exists() {
            eprintln!("[testly] File not found: {}", path_str);
            continue;
        }
        match parser::parse_file(path) {
            Ok(tf) => test_files.push(tf),
            Err(e) => eprintln!("[testly] Parse error in {}: {}", path_str, e),
        }
    }

    if test_files.is_empty() {
        eprintln!("[testly] No valid test files to run.");
        return Ok(());
    }

    let total = test_files.len();
    println!("[testly] Running {} test file(s) in parallel...\n", total);

    let results = runner::run_all(test_files);

    // Print summary
    println!("\n{}", "=".repeat(70));
    println!("  TESTLY RESULTS");
    println!("{}", "=".repeat(70));

    let mut passed = 0;
    let mut failed = 0;

    for result in &results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        let desc = result.description.as_deref().unwrap_or(&result.file);
        println!(
            "  [{}] {} ({}ms)",
            status, desc, result.duration_ms
        );

        if let Some(ref err) = result.error {
            println!("        Error: {}", err);
        }

        if result.passed {
            passed += 1;
        } else {
            failed += 1;
        }
    }

    println!("{}", "-".repeat(70));
    println!("  Total: {} | Passed: {} | Failed: {}", total, passed, failed);
    println!("{}", "=".repeat(70));

    if failed > 0 {
        // Print full logs for failed tests
        for result in &results {
            if !result.passed {
                println!("\n--- Log: {} ---", result.file);
                for line in &result.log {
                    println!("  {}", line);
                }
            }
        }
        std::process::exit(1);
    }

    Ok(())
}

fn run_direct(app_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[testly] Launching: {}", app_path);

    let (_child, hwnd) = launcher::launch_and_find_window(app_path)?;
    println!("[testly] Listing UI components...\n");

    inspector::list_components(hwnd, &mut |msg| println!("{}", msg))?;

    Ok(())
}
