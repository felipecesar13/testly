//! Robust multi-strategy automation module for Windows desktop applications.
//!
//! This module provides a trait-based abstraction over multiple UI automation
//! strategies (UI Automation API, pure Win32 messaging, etc.) together with a
//! fallback manager that transparently retries failed operations using the next
//! available strategy.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │          AutomationManager           │  ← high-level entry point
//! │  (fallback logic + strategy routing) │
//! ├──────────────────────────────────────┤
//! │  detection  │ detects window/control │
//! │             │ context and recommends │
//! │             │ the best strategy      │
//! ├──────────────────────────────────────┤
//! │  Strategies (impl AutomationStrategy)│
//! │  ┌─────────────┐ ┌────────────────┐ │
//! │  │  UIA impl   │ │  Win32 impl    │ │
//! │  └─────────────┘ └────────────────┘ │
//! └──────────────────────────────────────┘
//! ```
//!
//! # Adding a new strategy
//!
//! 1. Create a new file `src/automation/my_impl.rs`.
//! 2. Implement [`AutomationStrategy`] for your struct.
//! 3. Register it in [`fallback::AutomationManager::new`] (or let detection
//!    pick it up dynamically).

pub mod detection;
pub mod fallback;
pub mod uiautomation_impl;
pub mod win32_impl;

use windows::Win32::Foundation::HWND;

/// Errors produced by automation strategies.
pub type StrategyError = Box<dyn std::error::Error + Send + Sync>;

/// Result type used by automation strategies.
pub type StrategyResult<T = ()> = Result<T, StrategyError>;

// ─── Core trait ──────────────────────────────────────────────────────────────

/// Trait that every automation strategy must implement.
///
/// Each method receives a window handle, the relevant parameters, and a logging
/// callback so that the caller can capture detailed information without coupling
/// to a specific logging framework.
pub trait AutomationStrategy: Send + Sync {
    /// Human-readable name of the strategy (e.g. `"UIAutomation"`, `"Win32"`).
    fn name(&self) -> &str;

    /// List all UI components/controls visible in the window.
    fn list_components(
        &self,
        hwnd: HWND,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult;

    /// Find a control by type and name, then click/invoke it.
    fn find_and_click(
        &self,
        hwnd: HWND,
        control_type: &str,
        name: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult;

    /// Find a text-input control by name/id and fill it with `value`.
    fn find_and_fill(
        &self,
        hwnd: HWND,
        field_name: &str,
        value: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult;
}
