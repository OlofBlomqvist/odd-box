//! Custom page registration for odd-box.
//!
//! This module collects all odd-box-specific GUI pages and TUI tabs
//! and exposes them via simple constructor functions that `main.rs`
//! passes to the agent library's `GuiOptions` / `TuiOptions`.

mod updates;

/// Return the list of custom GUI pages to register with the agent GUI.
///
/// These pages appear in the sidebar after the built-in agent pages.
/// Each entry must implement `cruma::gui::CustomPage`.
pub fn custom_gui_pages() -> Vec<Box<dyn cruma::gui::CustomPage>> {
    vec![Box::new(updates::UpdatesPage::new())]
}

/// Return the list of custom TUI tabs to register with the agent TUI.
///
/// Each entry must implement `cruma::tui::CustomTuiTab`.
#[allow(dead_code)]
pub fn custom_tui_tabs() -> Vec<Box<dyn cruma::tui::CustomTuiTab>> {
    vec![
        // Add custom TUI tabs here in the future
    ]
}
