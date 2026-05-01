use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TextInsertionRequest {
    pub text: String,
    pub mode: TextInsertionMode,
    pub restore_clipboard: bool,
    pub clipboard_snapshot: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInsertionMode {
    CopyOnly,
    BestEffortVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertOutcome {
    Typed,
    Pasted,
    Copied,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertMethod {
    ClipboardOnly,
    ClipboardPaste,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTargetContext {
    pub platform: String,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextInsertionResult {
    pub outcome: InsertOutcome,
    pub method: InsertMethod,
    pub verified: bool,
    pub clipboard_restored: bool,
    pub target_context: Option<ActiveTargetContext>,
    pub message: String,
}

impl TextInsertionResult {
    pub fn overlay_state(&self) -> &'static str {
        match self.outcome {
            InsertOutcome::Typed => "typed",
            InsertOutcome::Pasted => "pasted",
            InsertOutcome::Copied => "copied",
            InsertOutcome::Blocked => "blocked",
            InsertOutcome::Failed => "error",
        }
    }
}

pub fn read_clipboard() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("pbpaste")
            .output()
            .map_err(|error| format!("Could not read clipboard: {error}"))?;
        if !output.status.success() {
            return Err("pbpaste failed to read the clipboard.".into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Clipboard snapshot is currently available on macOS only.".into())
    }
}

pub fn insert_text(request: TextInsertionRequest) -> TextInsertionResult {
    let target_context = capture_target_context();

    if request.text.trim().is_empty() {
        return TextInsertionResult {
            outcome: InsertOutcome::Failed,
            method: InsertMethod::Unsupported,
            verified: false,
            clipboard_restored: false,
            target_context,
            message: "Dictation produced no text to insert.".into(),
        };
    }

    match request.mode {
        TextInsertionMode::CopyOnly => copy_only(&request.text, target_context),
        TextInsertionMode::BestEffortVerified => best_effort_verified(request, target_context),
    }
}

fn copy_only(text: &str, target_context: Option<ActiveTargetContext>) -> TextInsertionResult {
    match write_clipboard(text) {
        Ok(()) => TextInsertionResult {
            outcome: InsertOutcome::Copied,
            method: InsertMethod::ClipboardOnly,
            verified: true,
            clipboard_restored: false,
            target_context,
            message: "Copied dictation to the clipboard.".into(),
        },
        Err(error) => TextInsertionResult {
            outcome: InsertOutcome::Failed,
            method: InsertMethod::ClipboardOnly,
            verified: false,
            clipboard_restored: false,
            target_context,
            message: error,
        },
    }
}

#[cfg(target_os = "macos")]
fn best_effort_verified(
    request: TextInsertionRequest,
    target_context: Option<ActiveTargetContext>,
) -> TextInsertionResult {
    if !minutes_core::hotkey_macos::is_accessibility_trusted() {
        return copy_after_block(request, target_context, "Accessibility permission is required to type into the active app. Copied dictation instead.");
    }

    let before_value = focused_ax_value().ok();

    match paste_via_clipboard(&request.text) {
        Ok(()) => {
            let verified = focused_ax_value().ok().is_some_and(|after| {
                before_value.as_ref() != Some(&after) && after.contains(&request.text)
            });
            let restored = restore_clipboard_if_requested(
                request.restore_clipboard,
                request.clipboard_snapshot.as_deref(),
            );
            TextInsertionResult {
                outcome: if verified {
                    InsertOutcome::Typed
                } else {
                    InsertOutcome::Pasted
                },
                method: InsertMethod::ClipboardPaste,
                verified,
                clipboard_restored: restored,
                target_context,
                message: if verified {
                    "Typed dictation into the active app.".into()
                } else {
                    "Pasted dictation into the active app.".into()
                },
            }
        }
        Err(error) => {
            tracing::warn!(error = %error, "dictation paste automation failed");
            TextInsertionResult {
                outcome: InsertOutcome::Copied,
                method: InsertMethod::ClipboardOnly,
                verified: true,
                clipboard_restored: false,
                target_context,
                message: "Could not type into the active app. Copied dictation instead.".into(),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn best_effort_verified(
    request: TextInsertionRequest,
    target_context: Option<ActiveTargetContext>,
) -> TextInsertionResult {
    copy_after_block(
        request,
        target_context,
        "Typing into apps is not implemented on this platform. Copied dictation instead.",
    )
}

fn copy_after_block(
    request: TextInsertionRequest,
    target_context: Option<ActiveTargetContext>,
    message: &str,
) -> TextInsertionResult {
    match write_clipboard(&request.text) {
        Ok(()) => TextInsertionResult {
            outcome: InsertOutcome::Blocked,
            method: InsertMethod::ClipboardOnly,
            verified: true,
            clipboard_restored: false,
            target_context,
            message: message.into(),
        },
        Err(error) => TextInsertionResult {
            outcome: InsertOutcome::Failed,
            method: InsertMethod::ClipboardOnly,
            verified: false,
            clipboard_restored: false,
            target_context,
            message: error,
        },
    }
}

fn restore_clipboard_if_requested(restore: bool, snapshot: Option<&str>) -> bool {
    if !restore {
        return false;
    }
    let Some(snapshot) = snapshot else {
        return false;
    };
    std::thread::sleep(Duration::from_millis(150));
    write_clipboard(snapshot).is_ok()
}

fn write_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start pbcopy: {error}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|error| format!("Could not write to clipboard: {error}"))?;
        }
        let status = child
            .wait()
            .map_err(|error| format!("Could not finish clipboard write: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("pbcopy failed to update the clipboard.".into())
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("Clipboard insertion is not implemented on this platform.".into())
    }
}

#[cfg(target_os = "macos")]
fn paste_via_clipboard(text: &str) -> Result<(), String> {
    write_clipboard(text)?;
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
        .output()
        .map_err(|error| format!("Could not run paste automation: {error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(if stderr.trim().is_empty() {
            "Paste automation failed.".into()
        } else {
            format!("Paste automation failed: {}", stderr.trim())
        })
    }
}

fn capture_target_context() -> Option<ActiveTargetContext> {
    #[cfg(target_os = "macos")]
    {
        let app_name = frontmost_app_name().ok();
        Some(ActiveTargetContext {
            platform: "macos".into(),
            app_name,
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Some(ActiveTargetContext {
            platform: std::env::consts::OS.into(),
            app_name: None,
        })
    }
}

#[cfg(target_os = "macos")]
fn frontmost_app_name() -> Result<String, String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(
            r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
        )
        .output()
        .map_err(|error| format!("Could not query frontmost app: {error}"))?;
    if !output.status.success() {
        return Err("Could not query frontmost app.".into());
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        Err("Frontmost app query returned no app.".into())
    } else {
        Ok(name)
    }
}

#[cfg(target_os = "macos")]
fn focused_ax_value() -> Result<String, String> {
    macos_ax::focused_value()
}

#[cfg(target_os = "macos")]
mod macos_ax {
    use std::ffi::{c_char, c_void, CString};
    use std::ptr;

    type AXError = i32;
    type AXUIElementRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFTypeRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type Boolean = u8;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFStringGetCString(
            the_string: CFStringRef,
            buffer: *mut c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> Boolean;
        fn CFRelease(cf: CFTypeRef);
    }

    pub fn focused_value() -> Result<String, String> {
        let system = unsafe { AXUIElementCreateSystemWide() };
        if system.is_null() {
            return Err("Could not create system accessibility element.".into());
        }

        let focused_attr = cfstring("AXFocusedUIElement")?;
        let mut focused: CFTypeRef = ptr::null();
        let focused_err =
            unsafe { AXUIElementCopyAttributeValue(system, focused_attr, &mut focused) };
        unsafe { CFRelease(focused_attr) };
        if focused_err != 0 || focused.is_null() {
            return Err(format!(
                "Could not read focused accessibility element (AX error {focused_err})."
            ));
        }

        let value = copy_string_attribute(focused.cast(), "AXValue");
        unsafe { CFRelease(focused) };
        value
    }

    fn copy_string_attribute(element: AXUIElementRef, name: &str) -> Result<String, String> {
        let attr = cfstring(name)?;
        let mut value: CFTypeRef = ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(element, attr, &mut value) };
        unsafe { CFRelease(attr) };
        if err != 0 || value.is_null() {
            return Err(format!(
                "Could not read AX attribute {name} (AX error {err})."
            ));
        }
        let string = cfstring_to_string(value.cast());
        unsafe { CFRelease(value) };
        string
    }

    fn cfstring(value: &str) -> Result<CFStringRef, String> {
        let c_string = CString::new(value)
            .map_err(|_| "Accessibility string contained an interior NUL byte.".to_string())?;
        let cf = unsafe {
            CFStringCreateWithCString(ptr::null(), c_string.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        };
        if cf.is_null() {
            Err("Could not create CoreFoundation string.".into())
        } else {
            Ok(cf)
        }
    }

    fn cfstring_to_string(value: CFStringRef) -> Result<String, String> {
        let mut buffer = vec![0i8; 8192];
        let ok = unsafe {
            CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                buffer.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
            )
        };
        if ok == 0 {
            return Err("AX string value was not readable as UTF-8.".into());
        }
        let nul = buffer
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(buffer.len());
        let bytes = buffer[..nul]
            .iter()
            .map(|byte| *byte as u8)
            .collect::<Vec<_>>();
        String::from_utf8(bytes).map_err(|error| format!("AX value was not UTF-8: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_result_maps_to_overlay_state() {
        let result = TextInsertionResult {
            outcome: InsertOutcome::Typed,
            method: InsertMethod::ClipboardPaste,
            verified: true,
            clipboard_restored: true,
            target_context: None,
            message: String::new(),
        };
        assert_eq!(result.overlay_state(), "typed");
    }

    #[test]
    fn failed_insertion_maps_to_error_state() {
        let result = TextInsertionResult {
            outcome: InsertOutcome::Failed,
            method: InsertMethod::Unsupported,
            verified: false,
            clipboard_restored: false,
            target_context: None,
            message: String::new(),
        };
        assert_eq!(result.overlay_state(), "error");
    }
}
