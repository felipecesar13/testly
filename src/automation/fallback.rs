//! Fallback manager — strategy routing with automatic retry.
//!
//! [`AutomationManager`] holds an ordered list of [`AutomationStrategy`]
//! implementations.  When an operation fails on the current strategy it
//! transparently retries with the next one, logging every switch.
//!
//! The detection module can optionally reorder strategies based on the target
//! window's characteristics (e.g. prefer Win32 for Delphi apps).

use windows::Win32::Foundation::HWND;

use super::detection;
use super::{AutomationStrategy, StrategyResult};

/// Orchestrates multiple [`AutomationStrategy`] implementations with fallback.
pub struct AutomationManager {
    strategies: Vec<Box<dyn AutomationStrategy>>,
}

impl AutomationManager {
    /// Create a manager with the given strategies in default priority order.
    pub fn new(strategies: Vec<Box<dyn AutomationStrategy>>) -> Self {
        Self { strategies }
    }

    /// Create a manager pre-loaded with all built-in strategies.
    pub fn with_defaults() -> Self {
        use super::uiautomation_impl::UiAutomationStrategy;
        use super::win32_impl::Win32Strategy;

        Self::new(vec![
            Box::new(UiAutomationStrategy::new()),
            Box::new(Win32Strategy::new()),
        ])
    }

    /// Return the ordered strategy names for inspection / logging.
    pub fn strategy_names(&self) -> Vec<&str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }

    // ── Fallback-aware operations ────────────────────────────────────────

    pub fn list_components(
        &self,
        hwnd: HWND,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        let order = self.resolve_order(hwnd, log);
        self.try_each(&order, log, |strategy, log| {
            strategy.list_components(hwnd, log)
        })
    }

    pub fn find_and_click(
        &self,
        hwnd: HWND,
        control_type: &str,
        name: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        let order = self.resolve_order(hwnd, log);
        self.try_each(&order, log, |strategy, log| {
            strategy.find_and_click(hwnd, control_type, name, log)
        })
    }

    pub fn find_and_fill(
        &self,
        hwnd: HWND,
        field_name: &str,
        value: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        let order = self.resolve_order(hwnd, log);
        self.try_each(&order, log, |strategy, log| {
            strategy.find_and_fill(hwnd, field_name, value, log)
        })
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    /// Determine strategy execution order.  Uses the detection module to
    /// inspect the target window and recommend an ordering; strategies not
    /// present in the recommendation are appended at the end.
    fn resolve_order<'a>(
        &'a self,
        hwnd: HWND,
        log: &mut dyn FnMut(&str),
    ) -> Vec<&'a dyn AutomationStrategy> {
        let info = detection::detect_window(hwnd, log);
        let recommended = detection::recommend_strategy_order(&info, log);

        let mut ordered: Vec<&dyn AutomationStrategy> = Vec::new();

        // Place recommended strategies first.
        for name in &recommended {
            if let Some(s) = self.strategies.iter().find(|s| s.name() == name.as_str()) {
                ordered.push(s.as_ref());
            }
        }

        // Append any remaining strategies not already included.
        for s in &self.strategies {
            if !ordered.iter().any(|o| std::ptr::eq(*o, s.as_ref())) {
                ordered.push(s.as_ref());
            }
        }

        log(&format!(
            "[fallback] Strategy order: {:?}",
            ordered.iter().map(|s| s.name()).collect::<Vec<_>>()
        ));

        ordered
    }

    /// Try `op` on each strategy in `order`.  Returns the first success or the
    /// last error if all strategies fail.
    fn try_each<F>(
        &self,
        order: &[&dyn AutomationStrategy],
        log: &mut dyn FnMut(&str),
        op: F,
    ) -> StrategyResult
    where
        F: Fn(&dyn AutomationStrategy, &mut dyn FnMut(&str)) -> StrategyResult,
    {
        let mut last_err: Option<Box<dyn std::error::Error + Send + Sync>> = None;

        for (i, strategy) in order.iter().enumerate() {
            log(&format!(
                "[fallback] Trying strategy {}/{}: {}",
                i + 1,
                order.len(),
                strategy.name()
            ));

            match op(*strategy, log) {
                Ok(()) => {
                    log(&format!(
                        "[fallback] Strategy '{}' succeeded.",
                        strategy.name()
                    ));
                    return Ok(());
                }
                Err(e) => {
                    log(&format!(
                        "[fallback] Strategy '{}' failed: {}",
                        strategy.name(),
                        e
                    ));
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| "All automation strategies failed".into()))
    }
}
