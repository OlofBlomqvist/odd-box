//! System tray icon implementation for macOS and Windows.
//!
//! This module provides a system tray icon with a context menu containing
//! a Show/Hide toggle and Quit option. On macOS, it uses the `tray-icon` crate
//! which requires initialization on the main thread.
//!
//! The tray icon is procedurally generated as a bold isometric box shape
//! that remains clearly visible at small sizes (32x32 or 22x22 pixels).

#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::{error, info, trace};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApplication;
#[cfg(target_os = "macos")]
use cocoa::base::nil;

const ICON_SIZE: u32 = 32;

// On macOS, we use a template-style icon (black + alpha) that the system
// automatically tints to match the menu bar appearance.
// On other platforms, we use a colored icon.

// ---------------------------------------------------------------------------
// Procedural isometric box icon
// ---------------------------------------------------------------------------
//
// A bold, simplified isometric box drawn procedurally at 32×32.
// The box has three visible faces (top, left, right) with thick edges
// that remain visible even at small sizes.
//
// The icon uses a template-style design (works well on both light and dark
// menu bars): solid shapes with good contrast.

// Box geometry - isometric projection
const BOX_CENTER_X: f32 = 16.0;

// Isometric box vertices (pre-calculated for a box centered in 32x32)
// Top face diamond
const TOP_Y: f32 = 4.0;
const MID_Y: f32 = 12.0;
const BOTTOM_Y: f32 = 20.0;
const FLOOR_Y: f32 = 28.0;

const LEFT_X: f32 = 4.0;
const RIGHT_X: f32 = 28.0;

// Line thickness for bold edges
const LINE_THICKNESS: f32 = 2.5;

/// Commands that can be sent from the tray menu to the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Show the application window
    Show,
    /// Hide the application window
    Hide,
    /// Quit the application
    Quit,
}

/// State of the tray icon (for disabling during shutdown)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayState {
    /// Normal operation
    Active,
    /// Application is shutting down, menu items disabled
    ShuttingDown,
}

/// Handle to the system tray icon.
///
/// This handle provides methods to update the tray menu state and
/// receive commands from user interactions.
pub struct TrayHandle {
    tray: Arc<Mutex<TrayIcon>>,
    /// Receiver for commands from tray menu interactions
    pub command_rx: Receiver<TrayCommand>,
    /// The toggle menu item (Show/Hide) - kept for updating text
    toggle_item: MenuItem,
    /// The quit menu item - kept for disabling during shutdown
    quit_item: MenuItem,
    /// Current visibility state
    window_visible: Arc<Mutex<bool>>,
    /// Current tray state
    state: Arc<Mutex<TrayState>>,
}

impl TrayHandle {
    /// Create a new tray icon synchronously on the main thread.
    ///
    /// This MUST be called on the main thread, especially on macOS.
    ///
    /// Returns a TrayHandle that can be used to receive commands and update menu state.
    pub fn new(app_name: &str) -> Result<TrayHandle, String> {
        info!("Initializing system tray icon for '{}'", app_name);

        #[cfg(target_os = "macos")]
        {
            let current_thread = std::thread::current();
            let thread_name = current_thread.name().unwrap_or("<unnamed>");
            info!(
                "Current thread: {:?} (id: {:?})",
                thread_name,
                current_thread.id()
            );
            if thread_name != "main" && !thread_name.is_empty() {
                error!(
                    "WARNING: Tray icon may fail - not on main thread! Current thread: {}",
                    thread_name
                );
            }
        }

        #[cfg(target_os = "macos")]
        {
            info!("macOS detected: Configuring NSApplication for tray");
            unsafe {
                let app = NSApplication::sharedApplication(nil);
                if app == nil {
                    error!("Failed to get NSApplication shared instance");
                    return Err("Failed to initialize NSApplication".to_string());
                }
                info!("NSApplication configured for tray support");
            }
        }

        // Build the context menu with a single toggle item
        let menu = Menu::new();
        let toggle_item = MenuItem::new("Hide", true, None); // Start as "Hide" since window is visible
        let quit_item = MenuItem::new("Quit", true, None);

        // Clone items before adding to menu (for storing in handle to update later)
        let toggle_item_for_handle = toggle_item.clone();
        let quit_item_for_handle = quit_item.clone();

        let _ = menu.append(&toggle_item);
        let _ = menu.append(&quit_item);

        info!("Building tray icon");
        let icon = build_tray_icon()?;

        let mut builder = TrayIconBuilder::new()
            .with_tooltip(app_name)
            .with_menu(Box::new(menu))
            .with_icon(icon);

        // On macOS, mark the icon as a template so the system tints it appropriately
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_as_template(true);
        }

        let tray = match builder.build() {
            Ok(t) => {
                info!("Tray icon created successfully");
                t
            }
            Err(e) => {
                error!("Failed to create tray icon: {:?}", e);
                return Err(format!("Failed to create tray icon: {:?}", e));
            }
        };

        // On non-Linux platforms, show menu on left click as well
        #[cfg(not(target_os = "linux"))]
        tray.set_show_menu_on_left_click(true);

        // Create channel for sending commands to the application
        let (command_tx, command_rx): (Sender<TrayCommand>, Receiver<TrayCommand>) =
            mpsc::channel();

        let menu_rx = MenuEvent::receiver();
        let toggle_id = toggle_item.id().clone();
        let quit_id = quit_item.id().clone();

        // Track window visibility for determining which command to send
        let window_visible = Arc::new(Mutex::new(true));
        let window_visible_for_thread = window_visible.clone();

        // Track tray state for shutdown
        let state = Arc::new(Mutex::new(TrayState::Active));
        let state_for_thread = state.clone();

        info!("Starting tray menu event handler");

        // Spawn a thread to handle menu events
        std::thread::spawn(move || {
            trace!("Tray menu event loop started");
            loop {
                // Check if shutting down
                if *state_for_thread.lock().unwrap() == TrayState::ShuttingDown {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    continue;
                }

                match menu_rx.try_recv() {
                    Ok(event) => {
                        let id = event.id;
                        if id == toggle_id {
                            let visible = *window_visible_for_thread.lock().unwrap();
                            if visible {
                                info!("Tray: Hide clicked");
                                let _ = command_tx.send(TrayCommand::Hide);
                            } else {
                                info!("Tray: Show clicked");
                                let _ = command_tx.send(TrayCommand::Show);
                            }
                        } else if id == quit_id {
                            info!("Tray: Quit clicked");
                            let _ = command_tx.send(TrayCommand::Quit);
                            break;
                        }
                    }
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
            trace!("Tray menu event loop exiting");
        });

        Ok(TrayHandle {
            tray: Arc::new(Mutex::new(tray)),
            command_rx,
            toggle_item: toggle_item_for_handle,
            quit_item: quit_item_for_handle,
            window_visible,
            state,
        })
    }

    /// Update the toggle menu item text based on window visibility.
    ///
    /// Call this whenever the window visibility changes.
    pub fn set_window_visible(&self, visible: bool) {
        *self.window_visible.lock().unwrap() = visible;
        if visible {
            self.toggle_item.set_text("Hide");
        } else {
            self.toggle_item.set_text("Show");
        }
        trace!("Tray toggle label updated: visible={}", visible);
    }

    /// Set the tray to shutting down state - disables and grays out menu items.
    ///
    /// Call this when the application begins its shutdown sequence.
    pub fn set_shutting_down(&self) {
        *self.state.lock().unwrap() = TrayState::ShuttingDown;
        // Disable menu items
        self.toggle_item.set_enabled(false);
        self.quit_item.set_enabled(false);
        // Update text to indicate shutting down
        self.quit_item.set_text("Quitting...");
        // Update icon to grayed-out version
        if let Ok(gray_icon) = build_gray_tray_icon() {
            if let Ok(tray) = self.tray.lock() {
                #[cfg(target_os = "macos")]
                let _ = tray.set_icon_as_template(true);
                let _ = tray.set_icon(Some(gray_icon));
            }
        }
        info!("Tray menu disabled for shutdown");
    }
}

/// Build a grayed-out version of the tray icon for shutdown state
fn build_gray_tray_icon() -> Result<Icon, String> {
    let rgba = build_gray_box_icon_rgba();
    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)
        .map_err(|e| format!("Failed to create gray tray icon: {:?}", e))
}

/// Build a grayed-out procedural isometric box icon as RGBA data
fn build_gray_box_icon_rgba() -> Vec<u8> {
    let size = ICON_SIZE;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // On macOS, use template style (outline only, black + alpha but dimmed)
    // On other platforms, use grayed-out colored faces
    #[cfg(target_os = "macos")]
    let use_template = true;
    #[cfg(not(target_os = "macos"))]
    let use_template = false;

    // Grayed-out colors for non-macOS platforms
    const GRAY_EDGE_R: u8 = 120;
    const GRAY_EDGE_G: u8 = 120;
    const GRAY_EDGE_B: u8 = 120;
    const GRAY_TOP_R: u8 = 180;
    const GRAY_TOP_G: u8 = 180;
    const GRAY_TOP_B: u8 = 180;
    const GRAY_LEFT_R: u8 = 160;
    const GRAY_LEFT_G: u8 = 160;
    const GRAY_LEFT_B: u8 = 160;
    const GRAY_RIGHT_R: u8 = 140;
    const GRAY_RIGHT_G: u8 = 140;
    const GRAY_RIGHT_B: u8 = 140;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let idx = ((y * size + x) * 4) as usize;

            if use_template {
                // macOS template style: outline only but with reduced alpha for gray effect
                let alpha: u8 = if is_on_edge(px, py) {
                    128 // Dimmed edges for shutdown state
                } else {
                    0 // Everything else transparent
                };

                rgba[idx] = 0; // R
                rgba[idx + 1] = 0; // G
                rgba[idx + 2] = 0; // B
                rgba[idx + 3] = alpha;
            } else {
                // Grayed-out colored icon for Windows/Linux
                let (r, g, b, a) = if is_on_edge(px, py) {
                    (GRAY_EDGE_R, GRAY_EDGE_G, GRAY_EDGE_B, 255u8)
                } else if is_in_top_face(px, py) {
                    (GRAY_TOP_R, GRAY_TOP_G, GRAY_TOP_B, 255u8)
                } else if is_in_left_face(px, py) {
                    (GRAY_LEFT_R, GRAY_LEFT_G, GRAY_LEFT_B, 255u8)
                } else if is_in_right_face(px, py) {
                    (GRAY_RIGHT_R, GRAY_RIGHT_G, GRAY_RIGHT_B, 255u8)
                } else {
                    (0, 0, 0, 0u8)
                };

                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = a;
            }
        }
    }

    rgba
}

/// Check if a point is inside the top face (diamond shape)
fn is_in_top_face(px: f32, py: f32) -> bool {
    let cx = BOX_CENTER_X;

    // Top diamond vertices
    let v0 = (cx, TOP_Y);
    let v1 = (LEFT_X, MID_Y);
    let v2 = (cx, BOTTOM_Y);
    let v3 = (RIGHT_X, MID_Y);

    point_in_quad(px, py, v0, v1, v2, v3)
}

/// Check if a point is inside the left face (parallelogram)
fn is_in_left_face(px: f32, py: f32) -> bool {
    let cx = BOX_CENTER_X;

    // Left face vertices
    let v0 = (LEFT_X, MID_Y);
    let v1 = (LEFT_X, FLOOR_Y - 4.0);
    let v2 = (cx, FLOOR_Y + 4.0);
    let v3 = (cx, BOTTOM_Y);

    point_in_quad(px, py, v0, v1, v2, v3)
}

/// Check if a point is inside the right face (parallelogram)
fn is_in_right_face(px: f32, py: f32) -> bool {
    let cx = BOX_CENTER_X;

    // Right face vertices
    let v0 = (cx, BOTTOM_Y);
    let v1 = (cx, FLOOR_Y + 4.0);
    let v2 = (RIGHT_X, FLOOR_Y - 4.0);
    let v3 = (RIGHT_X, MID_Y);

    point_in_quad(px, py, v0, v1, v2, v3)
}

/// Check if point is in a quadrilateral using cross products
fn point_in_quad(
    px: f32,
    py: f32,
    v0: (f32, f32),
    v1: (f32, f32),
    v2: (f32, f32),
    v3: (f32, f32),
) -> bool {
    fn cross(o: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    }

    let p = (px, py);
    let c0 = cross(v0, v1, p);
    let c1 = cross(v1, v2, p);
    let c2 = cross(v2, v3, p);
    let c3 = cross(v3, v0, p);

    // All same sign means inside
    (c0 >= 0.0 && c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0)
        || (c0 <= 0.0 && c1 <= 0.0 && c2 <= 0.0 && c3 <= 0.0)
}

/// Check if point is near a line segment (for drawing edges)
fn is_near_line(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32) -> bool {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 0.0001 {
        let d = ((px - x0).powi(2) + (py - y0).powi(2)).sqrt();
        return d <= thickness;
    }

    let t = ((px - x0) * dx + (py - y0) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let closest_x = x0 + t * dx;
    let closest_y = y0 + t * dy;

    let dist = ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt();
    dist <= thickness
}

/// Check if point is on any of the box edges
fn is_on_edge(px: f32, py: f32) -> bool {
    let cx = BOX_CENTER_X;
    let t = LINE_THICKNESS;

    // Top face edges
    if is_near_line(px, py, cx, TOP_Y, LEFT_X, MID_Y, t) {
        return true;
    }
    if is_near_line(px, py, LEFT_X, MID_Y, cx, BOTTOM_Y, t) {
        return true;
    }
    if is_near_line(px, py, cx, BOTTOM_Y, RIGHT_X, MID_Y, t) {
        return true;
    }
    if is_near_line(px, py, RIGHT_X, MID_Y, cx, TOP_Y, t) {
        return true;
    }

    // Vertical edges
    if is_near_line(px, py, LEFT_X, MID_Y, LEFT_X, FLOOR_Y - 4.0, t) {
        return true;
    }
    if is_near_line(px, py, RIGHT_X, MID_Y, RIGHT_X, FLOOR_Y - 4.0, t) {
        return true;
    }
    if is_near_line(px, py, cx, BOTTOM_Y, cx, FLOOR_Y + 4.0, t) {
        return true;
    }

    // Bottom edges
    if is_near_line(px, py, LEFT_X, FLOOR_Y - 4.0, cx, FLOOR_Y + 4.0, t) {
        return true;
    }
    if is_near_line(px, py, cx, FLOOR_Y + 4.0, RIGHT_X, FLOOR_Y - 4.0, t) {
        return true;
    }

    false
}

/// Build the procedural isometric box icon as RGBA data
fn build_box_icon_rgba() -> Vec<u8> {
    let size = ICON_SIZE;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // On macOS, use template style (outline only, black + alpha)
    // On other platforms, use colored faces
    #[cfg(target_os = "macos")]
    let use_template = true;
    #[cfg(not(target_os = "macos"))]
    let use_template = false;

    // Colors for non-macOS platforms
    const EDGE_R: u8 = 40;
    const EDGE_G: u8 = 60;
    const EDGE_B: u8 = 100;
    const TOP_R: u8 = 140;
    const TOP_G: u8 = 180;
    const TOP_B: u8 = 230;
    const LEFT_R: u8 = 90;
    const LEFT_G: u8 = 130;
    const LEFT_B: u8 = 190;
    const RIGHT_R: u8 = 60;
    const RIGHT_G: u8 = 100;
    const RIGHT_B: u8 = 160;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let idx = ((y * size + x) * 4) as usize;

            if use_template {
                // macOS template style: outline only (no filled faces)
                // Just draw the edges as solid black - system will tint appropriately
                let alpha: u8 = if is_on_edge(px, py) {
                    255 // Solid edges only
                } else {
                    0 // Everything else transparent (no filled faces)
                };

                // Template images use black color, alpha determines visibility
                rgba[idx] = 0; // R
                rgba[idx + 1] = 0; // G
                rgba[idx + 2] = 0; // B
                rgba[idx + 3] = alpha;
            } else {
                // Colored icon for Windows/Linux
                let (r, g, b, a) = if is_on_edge(px, py) {
                    (EDGE_R, EDGE_G, EDGE_B, 255u8)
                } else if is_in_top_face(px, py) {
                    (TOP_R, TOP_G, TOP_B, 255u8)
                } else if is_in_left_face(px, py) {
                    (LEFT_R, LEFT_G, LEFT_B, 255u8)
                } else if is_in_right_face(px, py) {
                    (RIGHT_R, RIGHT_G, RIGHT_B, 255u8)
                } else {
                    (0, 0, 0, 0u8)
                };

                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = a;
            }
        }
    }

    trace!(
        "Generated procedural tray icon ({}x{}, template={})",
        ICON_SIZE,
        ICON_SIZE,
        use_template
    );

    rgba
}

fn build_tray_icon() -> Result<Icon, String> {
    let rgba = build_box_icon_rgba();
    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)
        .map_err(|e| format!("Failed to create tray icon: {:?}", e))
}