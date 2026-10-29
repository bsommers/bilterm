//! Cross-Platform System Clipboard Bridge (`CLIP-01`).
//!
//! Provides zero-dependency access to the host OS clipboard and an in-process clipboard buffer:
//! - **macOS**: Native `NSPasteboard` (`generalPasteboard`, `"public.utf8-plain-text"`).
//! - **Windows**: Native Win32 `OpenClipboard` / `GetClipboardData` / `SetClipboardData` (`CF_UNICODETEXT`).
//! - **Linux**: Wayland / X11 (`wl-copy` / `wl-paste` / `xclip` / `xsel`) when a display server is present.

use std::sync::Mutex;

static PROCESS_CLIPBOARD: Mutex<String> = Mutex::new(String::new());

#[cfg(not(test))]
fn is_automated_test_process() -> bool {
    if std::env::var_os("RUST_TEST_THREADS").is_some()
        || std::env::var_os("NEXTEST").is_some()
        || std::env::var_os("CARGO_TARGET_TMPDIR").is_some()
    {
        return true;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().and_then(|d| d.file_name()).map(|n| n == "deps"))
        .unwrap_or(false)
}

/// Writes `text` to the in-process clipboard buffer and the native OS system clipboard.
pub fn set_system_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }
    if let Ok(mut guard) = PROCESS_CLIPBOARD.lock() {
        *guard = text.to_string();
    }
    #[cfg(not(test))]
    if !is_automated_test_process() {
        os_set_clipboard(text);
    }
}

/// Reads text from the native OS system clipboard (falling back to the in-process clipboard buffer).
pub fn get_system_clipboard() -> Option<String> {
    #[cfg(not(test))]
    if !is_automated_test_process()
        && let Some(s) = os_get_clipboard()
        && !s.is_empty()
    {
        if let Ok(mut guard) = PROCESS_CLIPBOARD.lock() {
            *guard = s.clone();
        }
        return Some(s);
    }
    PROCESS_CLIPBOARD
        .lock()
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.clone())
}

#[cfg(all(target_os = "macos", not(test)))]
mod macos_pb {
    use std::ffi::{CStr, c_char, c_void};
    use std::ptr::null;

    type Id = *mut c_void;
    type Sel = *mut c_void;
    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CFRange {
        location: isize,
        length: isize,
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_STRING_ENCODING_MAC_ROMAN: u32 = 0;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(str: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithBytes(
            alloc: CFTypeRef,
            bytes: *const u8,
            num_bytes: isize,
            encoding: u32,
            is_external_representation: u8,
        ) -> CFStringRef;
        fn CFStringGetLength(the_string: CFStringRef) -> isize;
        fn CFStringGetCharacters(the_string: CFStringRef, range: CFRange, buffer: *mut u16);
        fn CFRelease(cf: CFTypeRef);
    }

    fn class(name: &CStr) -> Id {
        unsafe { objc_getClass(name.as_ptr()) }
    }
    fn sel(name: &CStr) -> Sel {
        unsafe { sel_registerName(name.as_ptr()) }
    }
    fn cfstring(s: &str) -> CFStringRef {
        unsafe {
            let cf = CFStringCreateWithBytes(
                null(),
                s.as_ptr(),
                s.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
                0,
            );
            if !cf.is_null() {
                cf
            } else {
                CFStringCreateWithBytes(
                    null(),
                    s.as_ptr(),
                    s.len() as isize,
                    K_CF_STRING_ENCODING_MAC_ROMAN,
                    0,
                )
            }
        }
    }
    fn from_cfstring(cf: CFStringRef) -> String {
        if cf.is_null() {
            return String::new();
        }
        let len = unsafe { CFStringGetLength(cf) };
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize];
        unsafe {
            CFStringGetCharacters(
                cf,
                CFRange {
                    location: 0,
                    length: len,
                },
                buf.as_mut_ptr(),
            );
        }
        String::from_utf16_lossy(&buf)
    }

    pub fn set_pb(text: &str) {
        unsafe {
            let pb_cls = class(c"NSPasteboard");
            if pb_cls.is_null() {
                return;
            }
            let f_id_0: unsafe extern "C" fn(Id, Sel) -> Id =
                std::mem::transmute(objc_msgSend as *const ());
            let f_isize_0: unsafe extern "C" fn(Id, Sel) -> isize =
                std::mem::transmute(objc_msgSend as *const ());
            let f_bool_id_id: unsafe extern "C" fn(Id, Sel, Id, Id) -> u8 =
                std::mem::transmute(objc_msgSend as *const ());

            let pb = f_id_0(pb_cls, sel(c"generalPasteboard"));
            if pb.is_null() {
                return;
            }
            let _ = f_isize_0(pb, sel(c"clearContents"));
            let cf_str = cfstring(text);
            let cf_type = cfstring("public.utf8-plain-text");
            if !cf_str.is_null() && !cf_type.is_null() {
                let _ = f_bool_id_id(
                    pb,
                    sel(c"setString:forType:"),
                    cf_str as Id,
                    cf_type as Id,
                );
            }
            if !cf_str.is_null() {
                CFRelease(cf_str);
            }
            if !cf_type.is_null() {
                CFRelease(cf_type);
            }
        }
    }

    pub fn get_pb() -> Option<String> {
        unsafe {
            let pb_cls = class(c"NSPasteboard");
            if pb_cls.is_null() {
                return None;
            }
            let f_id_0: unsafe extern "C" fn(Id, Sel) -> Id =
                std::mem::transmute(objc_msgSend as *const ());
            let f_id_id: unsafe extern "C" fn(Id, Sel, Id) -> Id =
                std::mem::transmute(objc_msgSend as *const ());

            let pb = f_id_0(pb_cls, sel(c"generalPasteboard"));
            if pb.is_null() {
                return None;
            }
            let cf_type = cfstring("public.utf8-plain-text");
            if cf_type.is_null() {
                return None;
            }
            let ns_str = f_id_id(pb, sel(c"stringForType:"), cf_type as Id);
            CFRelease(cf_type);
            if ns_str.is_null() {
                return None;
            }
            let s = from_cfstring(ns_str.cast());
            (!s.is_empty()).then_some(s)
        }
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn os_set_clipboard(text: &str) {
    macos_pb::set_pb(text);
}

#[cfg(all(target_os = "macos", not(test)))]
fn os_get_clipboard() -> Option<String> {
    macos_pb::get_pb()
}

#[cfg(all(windows, not(test)))]
fn os_set_clipboard(text: &str) {
    use std::ffi::c_void;
    use std::ptr::{copy_nonoverlapping, null_mut};

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(h_wnd_new_owner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(u_format: u32, h_mem: *mut c_void) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalAlloc(u_flags: u32, dw_bytes: usize) -> *mut c_void;
        fn GlobalLock(h_mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(h_mem: *mut c_void) -> i32;
    }

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let byte_len = wide.len() * 2;

    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return;
        }
        let _ = EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if !h_mem.is_null() {
            let ptr = GlobalLock(h_mem).cast::<u16>();
            if !ptr.is_null() {
                copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(h_mem);
                let _ = SetClipboardData(CF_UNICODETEXT, h_mem);
            }
        }
        let _ = CloseClipboard();
    }
}

#[cfg(all(windows, not(test)))]
fn os_get_clipboard() -> Option<String> {
    use std::ffi::c_void;
    use std::ptr::null_mut;

    const CF_UNICODETEXT: u32 = 13;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(h_wnd_new_owner: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(u_format: u32) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalLock(h_mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(h_mem: *mut c_void) -> i32;
    }

    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let h_mem = GetClipboardData(CF_UNICODETEXT);
        let mut out = None;
        if !h_mem.is_null() {
            let ptr = GlobalLock(h_mem).cast::<u16>();
            if !ptr.is_null() {
                let mut len = 0usize;
                while *ptr.add(len) != 0 && len < 1_048_576 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                out = Some(String::from_utf16_lossy(slice));
                let _ = GlobalUnlock(h_mem);
            }
        }
        let _ = CloseClipboard();
        out.filter(|s| !s.is_empty())
    }
}

#[cfg(all(not(any(windows, target_os = "macos")), not(test)))]
fn wait_child_bounded(child: &mut std::process::Child, max_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => return false,
        }
    }
}

#[cfg(all(not(any(windows, target_os = "macos")), not(test)))]
fn os_set_clipboard(text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && let Ok(mut child) = Command::new("wl-copy")
            .args(["--type", "text/plain"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = wait_child_bounded(&mut child, 120);
        return;
    }
    if std::env::var_os("DISPLAY").is_some() {
        for (prog, args) in [
            ("xclip", &["-selection", "clipboard"][..]),
            ("xsel", &["-ib"][..]),
        ] {
            if let Ok(mut child) = Command::new(prog)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                let _ = wait_child_bounded(&mut child, 120);
                return;
            }
        }
    }
}

#[cfg(all(not(any(windows, target_os = "macos")), not(test)))]
fn run_read_command_bounded(prog: &str, args: &[&str], max_ms: u64) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut buf = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_end(&mut buf);
                }
                let s = String::from_utf8_lossy(&buf).into_owned();
                return (!s.is_empty()).then_some(s);
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(all(not(any(windows, target_os = "macos")), not(test)))]
fn os_get_clipboard() -> Option<String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && let Some(s) = run_read_command_bounded("wl-paste", &["--no-newline"], 200)
    {
        return Some(s);
    }
    if std::env::var_os("DISPLAY").is_some() {
        for (prog, args) in [
            ("xclip", &["-selection", "clipboard", "-o"][..]),
            ("xsel", &["-ob"][..]),
        ] {
            if let Some(s) = run_read_command_bounded(prog, args, 200) {
                return Some(s);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clipboard_buffer_roundtrips() {
        set_system_clipboard("echo bilterm clipboard");
        assert_eq!(
            get_system_clipboard().as_deref(),
            Some("echo bilterm clipboard")
        );
    }
}
