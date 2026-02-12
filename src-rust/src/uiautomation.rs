//! UI Automation COM interface for Windows Terminal tab detection.
//!
//! Uses IUIAutomation to enumerate tabs, find the selected one,
//! and capture/match its RuntimeId.

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::Variant::*;
use windows::Win32::UI::Accessibility::*;

/// Get the RuntimeId string of the currently selected WT tab.
/// Returns empty string on failure.
pub fn get_selected_tab_runtime_id(hwnd: HWND) -> String {
    unsafe { get_selected_tab_runtime_id_inner(hwnd).unwrap_or_default() }
}

unsafe fn get_selected_tab_runtime_id_inner(hwnd: HWND) -> Result<String> {
    let automation: IUIAutomation = CoCreateInstance(
        &CUIAutomation as *const GUID,
        None,
        CLSCTX_INPROC_SERVER,
    )?;

    let element = automation.ElementFromHandle(hwnd)?;

    // Create condition: ControlType == TabItem
    let prop_id = UIA_ControlTypePropertyId;
    let val = VARIANT::from(UIA_TabItemControlTypeId.0);
    let condition = automation.CreatePropertyCondition(prop_id, &val)?;

    // Find all tab items
    let tabs = element.FindAll(TreeScope_Descendants, &condition)?;
    let count = tabs.Length()?;

    for i in 0..count {
        let tab = tabs.GetElement(i)?;

        // Check if this tab is selected
        let pattern: Result<IUIAutomationSelectionItemPattern> =
            tab.GetCurrentPatternAs(UIA_SelectionItemPatternId);
        if let Ok(pattern) = pattern {
            let selected = pattern.CurrentIsSelected()?;
            if selected.as_bool() {
                return get_runtime_id_string(&tab);
            }
        }
    }

    Ok(String::new())
}

/// Select a WT tab by matching its RuntimeId string.
/// Returns true if the tab was found and selected.
pub fn select_tab_by_runtime_id(hwnd: HWND, target_runtime_id: &str) -> bool {
    unsafe { select_tab_inner(hwnd, target_runtime_id).unwrap_or(false) }
}

unsafe fn select_tab_inner(hwnd: HWND, target_runtime_id: &str) -> Result<bool> {
    let automation: IUIAutomation = CoCreateInstance(
        &CUIAutomation as *const GUID,
        None,
        CLSCTX_INPROC_SERVER,
    )?;

    let element = automation.ElementFromHandle(hwnd)?;

    let prop_id = UIA_ControlTypePropertyId;
    let val = VARIANT::from(UIA_TabItemControlTypeId.0);
    let condition = automation.CreatePropertyCondition(prop_id, &val)?;

    let tabs = element.FindAll(TreeScope_Descendants, &condition)?;
    let count = tabs.Length()?;

    for i in 0..count {
        let tab = tabs.GetElement(i)?;
        let rid = get_runtime_id_string(&tab).unwrap_or_default();

        if rid == target_runtime_id {
            let pattern: Result<IUIAutomationSelectionItemPattern> =
                tab.GetCurrentPatternAs(UIA_SelectionItemPatternId);
            if let Ok(pattern) = pattern {
                let _ = pattern.Select();
                return Ok(true);
            }
        }
    }

    Ok(false)
}

unsafe fn get_runtime_id_string(element: &IUIAutomationElement) -> Result<String> {
    let sa_ptr = element.GetRuntimeId()?;
    if sa_ptr.is_null() {
        return Ok(String::new());
    }

    let lower = SafeArrayGetLBound(sa_ptr, 1)?;
    let upper = SafeArrayGetUBound(sa_ptr, 1)?;

    let mut parts = Vec::new();
    for i in lower..=upper {
        let mut val: i32 = 0;
        SafeArrayGetElement(
            sa_ptr,
            &i as *const i32 as *const _,
            &mut val as *mut i32 as *mut _,
        )?;
        parts.push(val.to_string());
    }

    SafeArrayDestroy(sa_ptr)?;

    Ok(parts.join("."))
}
