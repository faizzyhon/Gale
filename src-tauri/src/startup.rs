//! Windows "start with Windows" entry, and moving users off the old per-user install
//! (builds up to 0.3.0 installed into %LOCALAPPDATA%; the installer is now all-users in Program Files).

/// Same UpgradeCode as scripts/installer/gale.wxs: every Gale MSI, per-user or all-users, shares it.
#[cfg_attr(not(windows), allow(dead_code))]
const UPGRADE_CODE: &str = "{832F8633-BF40-4C8B-A0DD-40BAAD025CD2}";

/// Value for HKCU\…\Run\Gale. Quoted, because "C:\Program Files\…" contains a space.
pub fn run_command(exe: &str) -> String {
    format!("\"{exe}\" --background")
}

/// Points the "start with Windows" entry at this exe (the autostart plugin writes the path unquoted).
pub fn write_run_entry() {
    #[cfg(windows)]
    if let Ok(exe) = std::env::current_exe() {
        imp::set_run(&run_command(&exe.display().to_string()));
    }
}

/// Browser registry keys for the native messaging host. The per-user (HKCU) keys win over the all-users ones.
#[cfg_attr(not(windows), allow(dead_code))]
pub const NATIVE_HOST_KEYS: [&str; 2] =
    [r"Software\Google\Chrome\NativeMessagingHosts\com.gale.nmh", r"Software\Microsoft\Edge\NativeMessagingHosts\com.gale.nmh"];

/// Points Chrome and Edge at the helper next to this exe, for the current user. Leftovers from another
/// install (an old per-user build) would otherwise make the browser talk to a helper that trusts a different Gale.
pub fn register_native_host() {
    #[cfg(windows)]
    if let Some(manifest) = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join("com.gale.nmh.json"))).filter(|m| m.is_file()) {
        for key in NATIVE_HOST_KEYS {
            imp::set_default(key, &manifest.display().to_string());
        }
    }
}

/// Uninstalls any per-user Gale for this user, keeping their "start with Windows" choice.
/// Returns true if one was removed. Once the old install is gone this is a quick registry lookup.
pub fn remove_per_user_install() -> bool {
    #[cfg(windows)]
    {
        imp::remove_per_user_install()
    }
    #[cfg(not(windows))]
    false
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::time::Duration;
    use windows_sys::Win32::System::ApplicationInstallationAndServicing::{
        MsiEnumRelatedProductsW, MsiGetProductInfoExW, INSTALLPROPERTY_PRODUCTSTATE, MSIINSTALLCONTEXT_USERUNMANAGED,
    };
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::Registry::{RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ};
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE};

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const ERROR_INSTALL_ALREADY_RUNNING: i32 = 1618;
    const ERROR_SUCCESS_REBOOT_REQUIRED: i32 = 3010;

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn has_run() -> bool {
        let (key, name) = (w(RUN_KEY), w("Gale"));
        let mut size = 0u32;
        // SAFETY: NUL-terminated buffers; a null data pointer only queries the size.
        unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) == 0 }
    }

    fn set_value(key: &str, name: Option<&str>, value: &str) {
        let (key, name, data) = (w(key), name.map(w), w(value));
        let name_ptr = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()); // null = the key's default value
        // SAFETY: NUL-terminated buffers alive for the call; size is in bytes including the terminator.
        unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name_ptr, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32) };
    }

    pub fn set_run(cmd: &str) {
        set_value(RUN_KEY, Some("Gale"), cmd);
    }

    pub fn set_default(key: &str, value: &str) {
        set_value(key, None, value);
    }

    /// Ends Gale processes started from `dir` (an old per-user copy), never this one.
    fn close_copies_in(dir: &std::path::Path) {
        // SAFETY: snapshot and process handles are checked and closed; buffers are sized as the APIs require.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap as isize == -1 {
                return;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snap, &mut e) != 0;
            while more {
                if e.th32ProcessID != std::process::id() {
                    let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE, 0, e.th32ProcessID);
                    if !h.is_null() {
                        let (mut buf, mut len) = ([0u16; 1024], 1024u32);
                        if QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0 {
                            let path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
                            if path.starts_with(dir) {
                                TerminateProcess(h, 1);
                            }
                        }
                        CloseHandle(h);
                    }
                }
                more = Process32NextW(snap, &mut e) != 0;
            }
            CloseHandle(snap);
        }
    }

    /// Product codes of Gale installs that exist in this user's per-user context (not the all-users install).
    fn per_user_products() -> Vec<String> {
        let upgrade = w(super::UPGRADE_CODE);
        let mut out = Vec::new();
        for i in 0.. {
            let mut code = [0u16; 39];
            // SAFETY: the buffer holds a GUID string (38 chars + NUL) as the API requires.
            if unsafe { MsiEnumRelatedProductsW(upgrade.as_ptr(), 0, i, code.as_mut_ptr()) } != 0 {
                break;
            }
            let (mut val, mut len) = ([0u16; 8], 8u32);
            // Asked for this one context only: the call fails for a product installed for all users.
            // SAFETY: code is NUL-terminated; null user SID = current user; val/len describe a writable buffer.
            let installed = unsafe {
                MsiGetProductInfoExW(code.as_ptr(), std::ptr::null(), MSIINSTALLCONTEXT_USERUNMANAGED, INSTALLPROPERTY_PRODUCTSTATE, val.as_mut_ptr(), &mut len)
            } == 0
                && val[0] == u16::from(b'5'); // INSTALLSTATE_DEFAULT
            if installed {
                out.push(String::from_utf16_lossy(&code[..38]));
            }
        }
        out
    }

    pub fn remove_per_user_install() -> bool {
        // Old builds installed to %LOCALAPPDATA%\Programs\Gale. Act only when that copy exists and is not us.
        let Some(old_dir) = std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join(r"Programs\Gale")) else {
            return false;
        };
        let running_from_old = std::env::current_exe().is_ok_and(|e| e.starts_with(&old_dir));
        if running_from_old || !old_dir.join("gale.exe").is_file() {
            return false;
        }
        // An old copy may still be running (its startup entry launched it). It would keep the browser connection
        // and, holding Gale's single-instance lock, swallow this launch. Close it first.
        close_copies_in(&old_dir);
        let products = per_user_products();
        if products.is_empty() {
            return false;
        }
        let had_autostart = has_run();
        let mut removed = false;
        for code in products {
            // Started by the new installer, we may run while it is still finishing: wait for it (up to ~3 min).
            for _ in 0..90 {
                let status = Command::new("msiexec").args(["/x", &code, "/qn", "REBOOT=ReallySuppress"]).creation_flags(CREATE_NO_WINDOW).status();
                match status.ok().and_then(|s| s.code()) {
                    Some(ERROR_INSTALL_ALREADY_RUNNING) => std::thread::sleep(Duration::from_secs(2)),
                    Some(0) | Some(ERROR_SUCCESS_REBOOT_REQUIRED) => {
                        removed = true;
                        break;
                    }
                    _ => break,
                }
            }
        }
        // The old uninstaller deletes the startup entry; restore it for this copy if the user had it.
        if removed && had_autostart {
            super::write_run_entry();
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_command_quotes_paths_with_spaces() {
        assert_eq!(super::run_command(r"C:\Program Files\Gale\gale.exe"), r#""C:\Program Files\Gale\gale.exe" --background"#);
    }
}
