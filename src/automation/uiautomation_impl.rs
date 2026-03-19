//! UI Automation–based strategy.
//!
//! Wraps the Windows UI Automation COM API (`IUIAutomation`) to interact with
//! application controls.  This is the richest strategy and supports patterns
//! such as Invoke, Toggle, and Value.

use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::*;

use super::{AutomationStrategy, StrategyResult};

const RPC_E_CHANGED_MODE: u32 = 0x80010106;

/// Strategy backed by the Windows UI Automation API.
pub struct UiAutomationStrategy;

impl UiAutomationStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl AutomationStrategy for UiAutomationStrategy {
    fn name(&self) -> &str {
        "UIAutomation"
    }

    fn list_components(
        &self,
        hwnd: HWND,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        unsafe {
            init_com()?;
            let automation = create_automation()?;

            let root = element_from_handle_with_retry(&automation, hwnd, log)?;
            let condition = automation.RawViewCondition()?;
            let walker = automation.CreateTreeWalker(&condition)?;

            log(&format!(
                "{:<6} {:<20} {:<30} {:<25} {}",
                "#", "Type", "Name", "ClassName", "AutomationId"
            ));
            log(&"-".repeat(100));

            let counter = &mut 1u32;
            print_element(&root, &walker, 0, counter, log)?;

            log(&format!(
                "\n[testly] Total: {} components found.",
                *counter - 1
            ));
        }
        Ok(())
    }

    fn find_and_click(
        &self,
        hwnd: HWND,
        control_type_str: &str,
        name: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        unsafe {
            init_com()?;
            let automation = create_automation()?;

            let root = element_from_handle_with_retry(&automation, hwnd, log)?;

            let target_type = control_type_from_str(control_type_str)
                .ok_or_else(|| format!("Unknown control type: {}", control_type_str))?;

            let type_cond = automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &windows::core::VARIANT::from(target_type.0 as i32),
            )?;

            let name_cond = automation.CreatePropertyCondition(
                UIA_NamePropertyId,
                &windows::core::VARIANT::from(windows::core::BSTR::from(name)),
            )?;

            let combined = automation.CreateAndCondition(&type_cond, &name_cond)?;

            let element = root.FindFirst(TreeScope_Descendants, &combined);

            match element {
                Ok(el) => {
                    log(&format!(
                        "[testly] Found element: {} \"{}\"",
                        control_type_str, name
                    ));

                    if let Ok(pattern) = el.GetCurrentPattern(UIA_InvokePatternId) {
                        let invoke: IUIAutomationInvokePattern = pattern.cast()?;
                        invoke.Invoke()?;
                        log(&format!(
                            "[testly] Clicked: {} \"{}\" (Invoke)",
                            control_type_str, name
                        ));
                    } else if let Ok(pattern) = el.GetCurrentPattern(UIA_TogglePatternId) {
                        let toggle: IUIAutomationTogglePattern = pattern.cast()?;
                        toggle.Toggle()?;
                        log(&format!(
                            "[testly] Clicked: {} \"{}\" (Toggle)",
                            control_type_str, name
                        ));
                    } else {
                        return Err(format!(
                            "Element {} \"{}\" found but does not support Invoke or Toggle pattern",
                            control_type_str, name
                        )
                        .into());
                    }
                    Ok(())
                }
                Err(_) => Err(format!(
                    "Element not found: {} \"{}\"",
                    control_type_str, name
                )
                .into()),
            }
        }
    }

    fn find_and_fill(
        &self,
        hwnd: HWND,
        name: &str,
        value: &str,
        log: &mut dyn FnMut(&str),
    ) -> StrategyResult {
        unsafe {
            init_com()?;
            let automation = create_automation()?;

            let root = element_from_handle_with_retry(&automation, hwnd, log)?;

            let type_cond = automation.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &windows::core::VARIANT::from(UIA_EditControlTypeId.0 as i32),
            )?;

            let name_cond = automation.CreatePropertyCondition(
                UIA_NamePropertyId,
                &windows::core::VARIANT::from(windows::core::BSTR::from(name)),
            )?;

            let combined = automation.CreateAndCondition(&type_cond, &name_cond)?;

            let element = root
                .FindFirst(TreeScope_Descendants, &combined)
                .or_else(|_| {
                    let id_cond = automation.CreatePropertyCondition(
                        UIA_AutomationIdPropertyId,
                        &windows::core::VARIANT::from(windows::core::BSTR::from(name)),
                    )?;
                    let combined2 = automation.CreateAndCondition(&type_cond, &id_cond)?;
                    root.FindFirst(TreeScope_Descendants, &combined2)
                });

            match element {
                Ok(el) => {
                    log(&format!("[testly] Found input: \"{}\"", name));

                    if let Ok(pattern) = el.GetCurrentPattern(UIA_ValuePatternId) {
                        let value_pattern: IUIAutomationValuePattern = pattern.cast()?;
                        let bstr = windows::core::BSTR::from(value);
                        value_pattern.SetValue(&bstr)?;
                        log(&format!("[testly] Filled: \"{}\" = \"{}\"", name, value));
                    } else {
                        return Err(format!(
                            "Element \"{}\" found but does not support ValuePattern",
                            name
                        )
                        .into());
                    }
                    Ok(())
                }
                Err(_) => Err(format!("Edit element not found: \"{}\"", name).into()),
            }
        }
    }
}

// ─── Internal helpers (ported from inspector.rs) ─────────────────────────────

fn init_com() -> StrategyResult {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_ok() {
            return Ok(());
        }
        if (hr.0 as u32) == RPC_E_CHANGED_MODE {
            return Ok(());
        }
        hr.ok()?;
    }
    Ok(())
}

fn create_automation() -> Result<IUIAutomation, Box<dyn std::error::Error + Send + Sync>> {
    unsafe {
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
        Ok(automation)
    }
}

unsafe fn element_from_handle_with_retry(
    automation: &IUIAutomation,
    hwnd: HWND,
    log: &mut dyn FnMut(&str),
) -> Result<IUIAutomationElement, Box<dyn std::error::Error + Send + Sync>> {
    const MAX_ATTEMPTS: u32 = 10;
    const BASE_DELAY_MS: u64 = 500;

    unsafe {
        let mut last_err = None;

        // Strategy 1: ElementFromHandle with retry
        for attempt in 1..=MAX_ATTEMPTS {
            match automation.ElementFromHandle(hwnd) {
                Ok(el) => return Ok(el),
                Err(e) => {
                    let delay = BASE_DELAY_MS * attempt.min(4) as u64;
                    if attempt < MAX_ATTEMPTS {
                        log(&format!(
                            "[WARN] ElementFromHandle attempt {}/{} failed: {}. Retrying in {}ms...",
                            attempt,
                            MAX_ATTEMPTS,
                            e.message(),
                            delay
                        ));
                        std::thread::sleep(std::time::Duration::from_millis(delay));
                    }
                    last_err = Some(e);
                }
            }
        }

        // Strategy 2: Fallback via root element search
        log("[INFO] ElementFromHandle exhausted retries. Trying fallback via GetRootElement...");

        const FALLBACK_ATTEMPTS: u32 = 3;
        const FALLBACK_DELAY_MS: u64 = 1000;

        if let Ok(root) = automation.GetRootElement() {
            if let Ok(cond) = automation.CreatePropertyCondition(
                UIA_NativeWindowHandlePropertyId,
                &windows::core::VARIANT::from(hwnd.0 as i32),
            ) {
                for fb in 1..=FALLBACK_ATTEMPTS {
                    match root.FindFirst(TreeScope_Children, &cond) {
                        Ok(el) => {
                            log("[INFO] Fallback succeeded: element found via GetRootElement.");
                            return Ok(el);
                        }
                        Err(e) => {
                            if fb < FALLBACK_ATTEMPTS {
                                log(&format!(
                                    "[WARN] Fallback attempt {}/{} failed: {}. Retrying...",
                                    fb,
                                    FALLBACK_ATTEMPTS,
                                    e.message()
                                ));
                                std::thread::sleep(std::time::Duration::from_millis(
                                    FALLBACK_DELAY_MS,
                                ));
                            }
                        }
                    }
                }

                if let Ok(el) = root.FindFirst(TreeScope_Descendants, &cond) {
                    log("[INFO] Fallback succeeded: element found via descendant search.");
                    return Ok(el);
                }
            }
        }

        Err(last_err.unwrap().into())
    }
}

unsafe fn print_element(
    element: &IUIAutomationElement,
    walker: &IUIAutomationTreeWalker,
    depth: usize,
    counter: &mut u32,
    log: &mut dyn FnMut(&str),
) -> StrategyResult {
    unsafe {
        let name = element
            .CurrentName()
            .map(|b| b.to_string())
            .unwrap_or_default();
        let control_type = element
            .CurrentControlType()
            .unwrap_or(UIA_CONTROLTYPE_ID(0));
        let class_name = element
            .CurrentClassName()
            .map(|b| b.to_string())
            .unwrap_or_default();
        let automation_id = element
            .CurrentAutomationId()
            .map(|b| b.to_string())
            .unwrap_or_default();

        let indent = "  ".repeat(depth);
        let type_name = control_type_name(control_type);

        log(&format!(
            "{:<6} {}{:<20} {:<30} {:<25} {}",
            counter,
            indent,
            type_name,
            truncate(&name, 28),
            truncate(&class_name, 23),
            automation_id,
        ));
        *counter += 1;

        if let Ok(child) = walker.GetFirstChildElement(element) {
            if let Err(e) = print_element(&child, walker, depth + 1, counter, log) {
                log(&format!("[WARN] Skipping element due to error: {}", e));
            }

            let mut current = child;
            while let Ok(next) = walker.GetNextSiblingElement(&current) {
                if let Err(e) = print_element(&next, walker, depth + 1, counter, log) {
                    log(&format!("[WARN] Skipping element due to error: {}", e));
                }
                current = next;
            }
        }
    }

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

#[allow(non_upper_case_globals)]
fn control_type_name(ct: UIA_CONTROLTYPE_ID) -> &'static str {
    match ct {
        UIA_AppBarControlTypeId => "AppBar",
        UIA_ButtonControlTypeId => "Button",
        UIA_CalendarControlTypeId => "Calendar",
        UIA_CheckBoxControlTypeId => "CheckBox",
        UIA_ComboBoxControlTypeId => "ComboBox",
        UIA_CustomControlTypeId => "Custom",
        UIA_DataGridControlTypeId => "DataGrid",
        UIA_DataItemControlTypeId => "DataItem",
        UIA_DocumentControlTypeId => "Document",
        UIA_EditControlTypeId => "Edit",
        UIA_GroupControlTypeId => "Group",
        UIA_HeaderControlTypeId => "Header",
        UIA_HeaderItemControlTypeId => "HeaderItem",
        UIA_HyperlinkControlTypeId => "Hyperlink",
        UIA_ImageControlTypeId => "Image",
        UIA_ListControlTypeId => "List",
        UIA_ListItemControlTypeId => "ListItem",
        UIA_MenuBarControlTypeId => "MenuBar",
        UIA_MenuControlTypeId => "Menu",
        UIA_MenuItemControlTypeId => "MenuItem",
        UIA_PaneControlTypeId => "Pane",
        UIA_ProgressBarControlTypeId => "ProgressBar",
        UIA_RadioButtonControlTypeId => "RadioButton",
        UIA_ScrollBarControlTypeId => "ScrollBar",
        UIA_SemanticZoomControlTypeId => "SemanticZoom",
        UIA_SeparatorControlTypeId => "Separator",
        UIA_SliderControlTypeId => "Slider",
        UIA_SpinnerControlTypeId => "Spinner",
        UIA_SplitButtonControlTypeId => "SplitButton",
        UIA_StatusBarControlTypeId => "StatusBar",
        UIA_TabControlTypeId => "Tab",
        UIA_TabItemControlTypeId => "TabItem",
        UIA_TableControlTypeId => "Table",
        UIA_TextControlTypeId => "Text",
        UIA_ThumbControlTypeId => "Thumb",
        UIA_TitleBarControlTypeId => "TitleBar",
        UIA_ToolBarControlTypeId => "ToolBar",
        UIA_ToolTipControlTypeId => "ToolTip",
        UIA_TreeControlTypeId => "Tree",
        UIA_TreeItemControlTypeId => "TreeItem",
        UIA_WindowControlTypeId => "Window",
        _ => "Unknown",
    }
}

#[allow(non_upper_case_globals)]
fn control_type_from_str(s: &str) -> Option<UIA_CONTROLTYPE_ID> {
    match s {
        "AppBar" => Some(UIA_AppBarControlTypeId),
        "Button" => Some(UIA_ButtonControlTypeId),
        "Calendar" => Some(UIA_CalendarControlTypeId),
        "CheckBox" => Some(UIA_CheckBoxControlTypeId),
        "ComboBox" => Some(UIA_ComboBoxControlTypeId),
        "Custom" => Some(UIA_CustomControlTypeId),
        "DataGrid" => Some(UIA_DataGridControlTypeId),
        "DataItem" => Some(UIA_DataItemControlTypeId),
        "Document" => Some(UIA_DocumentControlTypeId),
        "Edit" => Some(UIA_EditControlTypeId),
        "Group" => Some(UIA_GroupControlTypeId),
        "Header" => Some(UIA_HeaderControlTypeId),
        "HeaderItem" => Some(UIA_HeaderItemControlTypeId),
        "Hyperlink" => Some(UIA_HyperlinkControlTypeId),
        "Image" => Some(UIA_ImageControlTypeId),
        "List" => Some(UIA_ListControlTypeId),
        "ListItem" => Some(UIA_ListItemControlTypeId),
        "MenuBar" => Some(UIA_MenuBarControlTypeId),
        "Menu" => Some(UIA_MenuControlTypeId),
        "MenuItem" => Some(UIA_MenuItemControlTypeId),
        "Pane" => Some(UIA_PaneControlTypeId),
        "ProgressBar" => Some(UIA_ProgressBarControlTypeId),
        "RadioButton" => Some(UIA_RadioButtonControlTypeId),
        "ScrollBar" => Some(UIA_ScrollBarControlTypeId),
        "SemanticZoom" => Some(UIA_SemanticZoomControlTypeId),
        "Separator" => Some(UIA_SeparatorControlTypeId),
        "Slider" => Some(UIA_SliderControlTypeId),
        "Spinner" => Some(UIA_SpinnerControlTypeId),
        "SplitButton" => Some(UIA_SplitButtonControlTypeId),
        "StatusBar" => Some(UIA_StatusBarControlTypeId),
        "Tab" => Some(UIA_TabControlTypeId),
        "TabItem" => Some(UIA_TabItemControlTypeId),
        "Table" => Some(UIA_TableControlTypeId),
        "Text" => Some(UIA_TextControlTypeId),
        "Thumb" => Some(UIA_ThumbControlTypeId),
        "TitleBar" => Some(UIA_TitleBarControlTypeId),
        "ToolBar" => Some(UIA_ToolBarControlTypeId),
        "ToolTip" => Some(UIA_ToolTipControlTypeId),
        "Tree" => Some(UIA_TreeControlTypeId),
        "TreeItem" => Some(UIA_TreeItemControlTypeId),
        "Window" => Some(UIA_WindowControlTypeId),
        _ => None,
    }
}
