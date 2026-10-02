// Standalone window-backend selection for minifb on Linux/BSD.
//
// minifb creates windows through either the native Wayland protocol or X11
// (XWayland when the session runs on Wayland) and always probes Wayland first.
// Its Wayland backend relies on the compositor implementing
// `zxdg_decoration_manager_v1` for window decorations; GNOME does not, so
// those windows come up without a title bar or minimize/maximize/close
// buttons. X11 windows are decorated by every mainstream window manager, so
// the default prefers X11 whenever a `DISPLAY` is reachable.
//
// Windows and macOS builds of minifb only have their native backend, so the
// preference is ignored there.

use anyhow::{Context, Result};
use clap::ValueEnum;

/// Window backend requested on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum WindowBackendArg {
    /// Prefer X11 (XWayland) when a display is reachable so the window manager
    /// supplies standard window buttons; fall back to Wayland otherwise.
    Auto,
    /// Force the X11 backend.
    X11,
    /// Force the native Wayland backend.
    Wayland,
}

/// Concrete backend used to create the emulator window.
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowBackend {
    X11,
    Wayland,
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
impl WindowBackend {
    /// Human-readable name used in logs and error messages.
    pub fn label(self) -> &'static str {
        match self {
            WindowBackend::X11 => "x11",
            WindowBackend::Wayland => "wayland",
        }
    }
}

/// Decide which backends to try for a request, most preferred first.
///
/// `auto` prefers X11 whenever `DISPLAY` is set: X11 windows (including those
/// served by XWayland) get standard window buttons from the desktop window
/// manager, while minifb's Wayland windows are only decorated on compositors
/// that implement the xdg-decoration protocol.
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
pub fn backend_attempts(
    requested: WindowBackendArg,
    has_x11_display: bool,
    has_wayland_display: bool,
) -> Result<Vec<WindowBackend>> {
    use anyhow::bail;

    match requested {
        WindowBackendArg::X11 => {
            if !has_x11_display {
                bail!(
                    "--window-backend x11 requires the DISPLAY environment variable \
                     (XWayland is needed on Wayland-only sessions)"
                );
            }
            Ok(vec![WindowBackend::X11])
        }
        WindowBackendArg::Wayland => {
            if !has_wayland_display {
                bail!(
                    "--window-backend wayland requires a Wayland session \
                     (WAYLAND_DISPLAY is not set)"
                );
            }
            Ok(vec![WindowBackend::Wayland])
        }
        WindowBackendArg::Auto => {
            let mut attempts = Vec::new();
            if has_x11_display {
                attempts.push(WindowBackend::X11);
            }
            if has_wayland_display {
                attempts.push(WindowBackend::Wayland);
            }
            if attempts.is_empty() {
                bail!(
                    "no display server found: set DISPLAY (X11/XWayland) \
                     or WAYLAND_DISPLAY (Wayland)"
                );
            }
            Ok(attempts)
        }
    }
}

/// Connection variables that must be hidden so minifb selects `backend`.
///
/// minifb always probes Wayland before X11 and falls back silently, so an X11
/// attempt hides the Wayland socket variables and a Wayland attempt hides
/// `DISPLAY` to prevent an unwanted X11 fallback.
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
pub fn env_vars_to_hide(backend: WindowBackend) -> &'static [&'static str] {
    match backend {
        WindowBackend::X11 => &["WAYLAND_DISPLAY", "WAYLAND_SOCKET"],
        WindowBackend::Wayland => &["DISPLAY"],
    }
}

/// Temporarily removes environment variables so minifb's fixed backend probe
/// order lands on the intended backend. Previous values are restored on drop.
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
pub struct HiddenEnvVars {
    saved: Vec<(String, Option<std::ffi::OsString>)>,
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
impl HiddenEnvVars {
    /// Hide `keys` from the process environment until the guard is dropped.
    pub fn hide(keys: &[&str]) -> Self {
        let saved = keys
            .iter()
            .map(|key| {
                let previous = std::env::var_os(key);
                std::env::remove_var(key);
                (key.to_string(), previous)
            })
            .collect();
        Self { saved }
    }
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
impl Drop for HiddenEnvVars {
    fn drop(&mut self) {
        for (key, previous) in self.saved.drain(..) {
            match previous {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

/// Parse the integer window scale out of an XSETTINGS property blob.
///
/// Layout (verified against a live GNOME/XWayland session): a 12-byte
/// header — byte order (1), padding (3), serial (4), record count (4) —
/// then records of `type:u8, pad:u8, name_len:u16, name, pad-to-4,
/// serial:u32, value`. Type 0 values are a 4-byte integer. The byte-order
/// byte is ignored: the X server is always local to the client
/// (XWayland/Xorg on the same machine), so native endianness matches; a
/// foreign-endian server would only produce out-of-range values, which are
/// rejected here.
///
/// Returns `None` when `Gdk/WindowScalingFactor` is missing or outside the
/// sane 1..=8 range, letting the caller fall back to other signals.
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn scale_from_xsettings_blob(blob: &[u8]) -> Option<u32> {
    const MIN_RECORD: usize = 16; // type+pad+name_len+empty name+pad+serial+value
    if blob.len() < 12 {
        return None;
    }
    let count = u32::from_ne_bytes(blob[8..12].try_into().ok()?) as usize;
    if count > blob.len() / MIN_RECORD {
        return None;
    }
    let mut offset = 12usize;
    for _ in 0..count {
        if offset + 4 > blob.len() {
            return None;
        }
        let value_type = blob[offset];
        let name_len = u16::from_ne_bytes([blob[offset + 2], blob[offset + 3]]) as usize;
        let name_start = offset + 4;
        let name_end = name_start + name_len;
        if name_end > blob.len() {
            return None;
        }
        let name = &blob[name_start..name_end];
        offset = (name_end + 3) & !3; // pad the name to a 4-byte boundary
        if offset + 4 > blob.len() {
            return None;
        }
        offset += 4; // last-change serial
        match value_type {
            0 => {
                if offset + 4 > blob.len() {
                    return None;
                }
                let value = i32::from_ne_bytes(blob[offset..offset + 4].try_into().ok()?);
                offset += 4;
                if name == b"Gdk/WindowScalingFactor" {
                    return u32::try_from(value).ok().filter(|v| (1..=8).contains(v));
                }
            }
            1 => {
                if offset + 4 > blob.len() {
                    return None;
                }
                let len = u32::from_ne_bytes(blob[offset..offset + 4].try_into().ok()?) as usize;
                offset += 4 + len;
                if offset > blob.len() {
                    return None;
                }
                offset = (offset + 3) & !3;
            }
            2 => {
                offset += 8; // color
                if offset > blob.len() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    None
}

/// Derive a window scale from an `Xft.dpi` resource line (96 dpi = 1x).
///
/// This is a fallback for sessions whose XSETTINGS manager does not export
/// `Gdk/WindowScalingFactor`; note it conflates text scaling with window
/// scaling, so it is only consulted when the primary signal is absent.
#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn scale_from_xft_dpi(resources: &str) -> Option<u32> {
    let line = resources
        .lines()
        .find(|line| line.trim_start().starts_with("Xft.dpi"))?;
    let (_, value) = line.split_once(':')?;
    let dpi: f64 = value.trim().parse().ok()?;
    let scale = (dpi / 96.0).round();
    if (1.0..=8.0).contains(&scale) {
        Some(scale as u32)
    } else {
        None
    }
}

/// Read an X window property as raw bytes, or `None` if it is missing or
/// not an 8-bit (byte) property.
#[cfg(all(unix, not(target_os = "macos")))]
fn x11_property_bytes(
    lib: &x11_dl::xlib::Xlib,
    display: *mut x11_dl::xlib::Display,
    window: std::os::raw::c_ulong,
    property: std::os::raw::c_ulong,
) -> Option<Vec<u8>> {
    use std::os::raw::{c_int, c_void};

    // SAFETY: `display`/`window`/`property` come from live Xlib calls on the
    // previous lines; `prop` is allocated by the X server and freed with
    // XFree on every path that returns it.
    unsafe {
        let mut actual_type: std::os::raw::c_ulong = 0;
        let mut actual_format: c_int = 0;
        let mut nitems: std::os::raw::c_ulong = 0;
        let mut bytes_after: std::os::raw::c_ulong = 0;
        let mut prop: *mut u8 = std::ptr::null_mut();
        // long_length counts 32-bit units; 4096 units (16 KiB) comfortably
        // covers the ~2 KiB XSETTINGS blob and the RESOURCE_MANAGER string.
        let status = (lib.XGetWindowProperty)(
            display,
            window,
            property,
            0,
            4096,
            0, // do not delete the property
            0, // AnyPropertyType
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut prop,
        );
        if status != 0 || prop.is_null() || actual_format != 8 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(prop, nitems as usize).to_vec();
        (lib.XFree)(prop as *mut c_void);
        Some(bytes)
    }
}

/// Read `Gdk/WindowScalingFactor` from the session's XSETTINGS manager.
#[cfg(all(unix, not(target_os = "macos")))]
fn read_xsettings_scale() -> Option<u32> {
    use x11_dl::xlib;

    let lib = xlib::Xlib::open().ok()?;
    // SAFETY: the display pointer is checked for null before use, and every
    // call below is a plain Xlib round-trip; the connection is closed on all
    // paths.
    unsafe {
        let display = (lib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let scale = (|| {
            let selection = (lib.XInternAtom)(display, c"_XSETTINGS_S0".as_ptr(), 0);
            if selection == 0 {
                return None;
            }
            let owner = (lib.XGetSelectionOwner)(display, selection);
            if owner == 0 {
                return None;
            }
            let property = (lib.XInternAtom)(display, c"_XSETTINGS_SETTINGS".as_ptr(), 0);
            if property == 0 {
                return None;
            }
            let blob = x11_property_bytes(&lib, display, owner, property)?;
            scale_from_xsettings_blob(&blob)
        })();
        (lib.XCloseDisplay)(display);
        scale
    }
}

/// Read `Xft.dpi` from the root window's RESOURCE_MANAGER resource.
#[cfg(all(unix, not(target_os = "macos")))]
fn read_xft_dpi_scale() -> Option<u32> {
    use x11_dl::xlib;

    let lib = xlib::Xlib::open().ok()?;
    // SAFETY: as in `read_xsettings_scale`; the property is requested from
    // the root window of a freshly opened local connection.
    unsafe {
        let display = (lib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let scale = (|| {
            let root = (lib.XDefaultRootWindow)(display);
            let property = (lib.XInternAtom)(display, c"RESOURCE_MANAGER".as_ptr(), 0);
            if property == 0 {
                return None;
            }
            let bytes = x11_property_bytes(&lib, display, root, property)?;
            scale_from_xft_dpi(&String::from_utf8_lossy(&bytes))
        })();
        (lib.XCloseDisplay)(display);
        scale
    }
}

/// Integer density of the XWayland X11 coordinate space: X11 pixels per
/// logical pixel (e.g. 2 on a 125 % desktop whose X screen is 3072×1728
/// over a 1536×864 logical desktop).
///
/// The frontend sizes windows in logical pixels (the size the Wayland
/// backend would have produced), so the X11 backend must multiply the
/// request by this factor or the window would cover only `1/density` of the
/// intended on-screen area. Prefers XSETTINGS `Gdk/WindowScalingFactor`
/// (exactly the logical→X11 factor GDK itself applies), falls back to
/// `Xft.dpi ÷ 96`, and defaults to 1 when neither signal is available.
#[cfg(all(unix, not(target_os = "macos")))]
fn xwayland_scale() -> u32 {
    read_xsettings_scale()
        .or_else(read_xft_dpi_scale)
        .unwrap_or(1)
}

/// Create the emulator window with the requested backend, logging the choice.
///
/// Only the Linux/BSD builds of minifb have multiple backends; Windows and
/// macOS always use their native windowing system and ignore the request.
pub fn create_window(
    requested: WindowBackendArg,
    title: &str,
    width: usize,
    height: usize,
    options: minifb::WindowOptions,
) -> Result<minifb::Window> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        create_window_posix(requested, title, width, height, options)
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = requested;
        minifb::Window::new(title, width, height, options)
            .context("failed to create emulator window")
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn create_window_posix(
    requested: WindowBackendArg,
    title: &str,
    width: usize,
    height: usize,
    options: minifb::WindowOptions,
) -> Result<minifb::Window> {
    use log::{info, warn};

    let has_x11_display = !std::env::var_os("DISPLAY").unwrap_or_default().is_empty();
    let has_wayland_display = !std::env::var_os("WAYLAND_DISPLAY")
        .unwrap_or_default()
        .is_empty()
        || !std::env::var_os("WAYLAND_SOCKET")
            .unwrap_or_default()
            .is_empty();
    let attempts = backend_attempts(requested, has_x11_display, has_wayland_display)?;

    let mut last_error = None;
    for backend in attempts {
        // The Wayland backend already works in logical pixels; only the X11
        // window needs the XWayland density multiplier. Under XWayland the
        // X11 coordinate space is denser than logical pixels (e.g. a 3072x1728
        // X screen over a 1536x864 logical desktop = 2x), so an X11 window
        // requested at logical size would cover only a fraction of the
        // intended on-screen area. Native X11 sessions keep 1:1 sizing.
        let (attempt_width, attempt_height) = match backend {
            WindowBackend::X11 => {
                let scale = if has_wayland_display {
                    xwayland_scale()
                } else {
                    1
                };
                if scale > 1 {
                    info!(
                        "XWayland X11 density {scale}x: opening {}x{} window \
                         for a {width}x{height} logical frame",
                        width * scale as usize,
                        height * scale as usize
                    );
                }
                (width * scale as usize, height * scale as usize)
            }
            WindowBackend::Wayland => (width, height),
        };
        // Window creation runs before any helper threads start, so briefly
        // reshaping the environment is safe; the guard restores it right away.
        let _hidden = HiddenEnvVars::hide(env_vars_to_hide(backend));
        match minifb::Window::new(title, attempt_width, attempt_height, options) {
            Ok(window) => {
                info!("Window backend: {}", backend.label());
                return Ok(window);
            }
            Err(error) => {
                warn!("window backend {} failed: {error}", backend.label());
                last_error = Some(error);
            }
        }
    }

    let error = last_error.expect("backend_attempts returned at least one backend");
    Err(error).context("failed to create emulator window")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_x11_when_both_displays_are_available() {
        let attempts = backend_attempts(WindowBackendArg::Auto, true, true).expect("auto resolves");
        assert_eq!(attempts, vec![WindowBackend::X11, WindowBackend::Wayland]);
    }

    #[test]
    fn auto_uses_wayland_without_an_x11_display() {
        let attempts =
            backend_attempts(WindowBackendArg::Auto, false, true).expect("auto resolves");
        assert_eq!(attempts, vec![WindowBackend::Wayland]);
    }

    #[test]
    fn auto_errors_without_any_display() {
        assert!(backend_attempts(WindowBackendArg::Auto, false, false).is_err());
    }

    #[test]
    fn explicit_backends_are_honored() {
        assert_eq!(
            backend_attempts(WindowBackendArg::X11, true, true).unwrap(),
            vec![WindowBackend::X11]
        );
        assert_eq!(
            backend_attempts(WindowBackendArg::Wayland, true, true).unwrap(),
            vec![WindowBackend::Wayland]
        );
    }

    #[test]
    fn explicit_x11_requires_a_display() {
        assert!(backend_attempts(WindowBackendArg::X11, false, true).is_err());
    }

    #[test]
    fn explicit_wayland_requires_a_wayland_session() {
        assert!(backend_attempts(WindowBackendArg::Wayland, true, false).is_err());
    }

    #[test]
    fn x11_attempts_hide_the_wayland_connection() {
        assert_eq!(
            env_vars_to_hide(WindowBackend::X11),
            &["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
        );
    }

    #[test]
    fn wayland_attempts_hide_the_x11_connection() {
        assert_eq!(env_vars_to_hide(WindowBackend::Wayland), &["DISPLAY"]);
    }

    #[test]
    fn hidden_env_vars_restore_previous_values() {
        // Use private names so parallel tests never fight over real session
        // variables such as DISPLAY.
        let key = "NICAIEMU_TEST_HIDDEN_VAR";
        std::env::set_var(key, "kept");
        {
            let _hidden = HiddenEnvVars::hide(&[key]);
            assert!(std::env::var_os(key).is_none());
        }
        assert_eq!(std::env::var(key).as_deref(), Ok("kept"));
        std::env::remove_var(key);
    }

    #[test]
    fn hidden_env_vars_restore_absent_values() {
        let key = "NICAIEMU_TEST_HIDDEN_ABSENT_VAR";
        std::env::remove_var(key);
        {
            let _hidden = HiddenEnvVars::hide(&[key]);
            assert!(std::env::var_os(key).is_none());
        }
        assert!(std::env::var_os(key).is_none());
    }

    // --- XWayland density detection (pure parsing, no X server needed) ---

    /// Build an XSETTINGS blob: 12-byte header (byte order, serial, count)
    /// followed by records matching the layout `scale_from_xsettings_blob`
    /// documents.
    fn xsettings_blob(count: u32, records: Vec<u8>) -> Vec<u8> {
        let mut blob = vec![0u8; 12];
        blob[8..12].copy_from_slice(&count.to_ne_bytes());
        blob.extend_from_slice(&records);
        blob
    }

    /// Integer record: type 0, name, 4-byte-aligned, serial, value.
    fn int_record(name: &str, value: i32) -> Vec<u8> {
        let mut record = vec![0u8, 0u8]; // type = integer, padding
        record.extend_from_slice(&(name.len() as u16).to_ne_bytes());
        record.extend_from_slice(name.as_bytes());
        while record.len() % 4 != 0 {
            record.push(0);
        }
        record.extend_from_slice(&0u32.to_ne_bytes()); // last-change serial
        record.extend_from_slice(&value.to_ne_bytes());
        record
    }

    /// String record: type 1, name, serial, length-prefixed padded value.
    fn string_record(name: &str, value: &str) -> Vec<u8> {
        let mut record = vec![1u8, 0u8]; // type = string, padding
        record.extend_from_slice(&(name.len() as u16).to_ne_bytes());
        record.extend_from_slice(name.as_bytes());
        while record.len() % 4 != 0 {
            record.push(0);
        }
        record.extend_from_slice(&0u32.to_ne_bytes()); // last-change serial
        record.extend_from_slice(&(value.len() as u32).to_ne_bytes());
        record.extend_from_slice(value.as_bytes());
        while record.len() % 4 != 0 {
            record.push(0);
        }
        record
    }

    #[test]
    fn xsettings_blob_reports_window_scaling_factor() {
        let mut records = string_record("Gtk/ThemeName", "Yaru");
        records.extend(int_record("Gdk/UnscaledDPI", 96 * 1024));
        records.extend(int_record("Gdk/WindowScalingFactor", 2));
        let blob = xsettings_blob(3, records);

        assert_eq!(scale_from_xsettings_blob(&blob), Some(2));
    }

    #[test]
    fn xsettings_blob_without_scaling_factor_is_none() {
        let blob = xsettings_blob(1, string_record("Gtk/ThemeName", "Yaru"));

        assert_eq!(scale_from_xsettings_blob(&blob), None);
    }

    #[test]
    fn xsettings_blob_rejects_out_of_range_scaling_factors() {
        for value in [0, -1, 9, i32::MAX] {
            let blob = xsettings_blob(1, int_record("Gdk/WindowScalingFactor", value));
            assert_eq!(
                scale_from_xsettings_blob(&blob),
                None,
                "scale {value} should be rejected"
            );
        }
    }

    #[test]
    fn xsettings_blob_rejects_truncated_input_without_panicking() {
        let blob = xsettings_blob(3, int_record("Gdk/WindowScalingFactor", 2));
        for cut in [0, 8, 11, 13, blob.len() - 1] {
            assert_eq!(scale_from_xsettings_blob(&blob[..cut]), None, "cut {cut}");
        }
    }

    #[test]
    fn xft_dpi_lines_convert_to_window_scale() {
        // Real RESOURCE_MANAGER content from a GNOME XWayland session.
        let resources =
            "*customization:\t-color\nXcursor.size:\t48\nXft.dpi:\t192\nXft.hinting:\t1\n";
        assert_eq!(scale_from_xft_dpi(resources), Some(2));
        assert_eq!(scale_from_xft_dpi("Xft.dpi:\t96\n"), Some(1));
        assert_eq!(scale_from_xft_dpi("Xft.dpi:\t144\n"), Some(2)); // rounds 1.5 -> 2
        assert_eq!(scale_from_xft_dpi("Xft.dpi:\tnot-a-number\n"), None);
        assert_eq!(scale_from_xft_dpi("Xft.dpi:\t12000\n"), None); // out of 1..=8
        assert_eq!(scale_from_xft_dpi("no dpi here\n"), None);
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn xwayland_scale_is_a_sane_factor() {
        // 1 on machines without a display server or XSETTINGS; the session
        // factor (e.g. 2) on a scaled XWayland desktop.
        assert!((1..=8).contains(&xwayland_scale()));
    }
}
