//! macOS-specific utilities for application appearance and behavior.
//!
//! This module provides:
//! - Setting the application dock icon at runtime (for command-line launches)
//! - Controlling the activation policy (show/hide in Dock)

#![allow(deprecated)]
#![allow(unexpected_cfgs)]

use tracing::warn;

/// macOS application activation policies.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ActivationPolicy {
    /// Regular app - appears in Dock and Cmd+Tab
    Regular = 0,
    /// Accessory app - doesn't appear in Dock, but can have windows
    Accessory = 1,
    /// Prohibited - pure background app, no UI
    #[allow(dead_code)]
    Prohibited = 2,
}

/// Sets the macOS application activation policy.
///
/// - `Regular`: App appears in Dock and Cmd+Tab switcher
/// - `Accessory`: App doesn't appear in Dock (useful when window is hidden but tray is active)
///
/// On non-macOS platforms, this function is a no-op.
#[allow(dead_code)]
pub fn set_activation_policy(#[allow(unused_variables)] policy: ActivationPolicy) {
    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::NSApplication;
        use cocoa::base::nil;
        use objc::{msg_send, sel, sel_impl};

        unsafe {
            let app = NSApplication::sharedApplication(nil);
            if app != nil {
                let _: () = msg_send![app, setActivationPolicy: policy as i64];
                tracing::trace!("Set macOS activation policy to {:?}", policy);
            }
        }
    }
}

/// Applies the default application icon on macOS.
///
/// This should be called early during GUI initialization to ensure the dock
/// icon is correct even when the app is launched from the command line.
///
/// On non-macOS platforms, this function is a no-op.
pub fn apply_default_icon() {
    #[cfg(target_os = "macos")]
    {
        if let Err(err) = set_icon_from_png_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/ob3.png"
        ))) {
            warn!(error = %err, "Failed to apply macOS app icon");
        }
    }
}

#[cfg(target_os = "macos")]
fn set_icon_from_png_bytes(png_bytes: &[u8]) -> Result<(), String> {
    use cocoa::appkit::{NSApplication, NSApplicationActivationPolicyRegular};
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let app = NSApplication::sharedApplication(nil);
        if app == nil {
            return Err("NSApplication is unavailable".to_string());
        }

        // Terminal-launched GUI binaries can otherwise inherit a terminal identity
        // in Dock/Cmd+Tab. Setting activation policy ensures proper app behavior.
        let _ = app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        let data: id = msg_send![
            class!(NSData),
            dataWithBytes: png_bytes.as_ptr() as *const std::ffi::c_void
            length: png_bytes.len()
        ];
        if data == nil {
            return Err("Failed to create NSData from icon bytes".to_string());
        }

        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithData: data];
        if image == nil {
            return Err("Failed to decode NSImage from PNG bytes".to_string());
        }

        app.setApplicationIconImage_(image);
    }

    Ok(())
}