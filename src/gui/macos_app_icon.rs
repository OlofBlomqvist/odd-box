//! macOS-specific utility to set the application dock icon at runtime.
//!
//! When launching a Rust GUI application from the terminal on macOS, the dock
//! icon defaults to the terminal's icon. This module uses Cocoa APIs to
//! explicitly set the application icon from embedded PNG bytes.

#![allow(deprecated)]
#![allow(unexpected_cfgs)]

use tracing::warn;

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