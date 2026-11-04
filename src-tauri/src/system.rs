//! Windows integration: Mark-of-the-Web, Microsoft Defender scanning, system shutdown.
//! Every external program here is a fixed path with fixed arguments; nothing comes from the network or UI.

use std::path::Path;

/// Tag a downloaded file as coming from the internet so SmartScreen/Office Protected View apply to it.
pub fn mark_of_the_web(path: &Path, url: &str, referrer: Option<&str>) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let mut ads = path.as_os_str().to_owned();
        ads.push(":Zone.Identifier");
        // Only http(s) URLs reach here; strip anything that could break the INI format.
        let clean = |s: &str| s.replace(['\r', '\n'], "");
        let mut body = format!("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={}\r\n", clean(url));
        if let Some(r) = referrer {
            body.push_str(&format!("ReferrerUrl={}\r\n", clean(r)));
        }
        std::fs::write(ads, body)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, url, referrer);
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum ScanResult {
    Clean,
    Threat,
    Unavailable(String),
}

/// Custom scan of one file with Microsoft Defender's command-line scanner.
pub async fn defender_scan(path: &Path) -> ScanResult {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
        let exe = Path::new(&pf).join(r"Windows Defender\MpCmdRun.exe");
        if !exe.exists() {
            return ScanResult::Unavailable("Microsoft Defender is not available".into());
        }
        let p = path.to_path_buf();
        let out = tokio::task::spawn_blocking(move || {
            std::process::Command::new(exe)
                .args(["-Scan", "-ScanType", "3", "-File"]) // Defender quarantines threats itself
                .arg(p)
                .creation_flags(CREATE_NO_WINDOW)
                .status()
        })
        .await;
        // MpCmdRun: 0 = no threats, 2 = threats found.
        match out {
            Ok(Ok(s)) if s.code() == Some(0) => ScanResult::Clean,
            Ok(Ok(s)) if s.code() == Some(2) => ScanResult::Threat,
            Ok(Ok(s)) => ScanResult::Unavailable(format!("scanner exited with {s}")),
            Ok(Err(e)) => ScanResult::Unavailable(e.to_string()),
            Err(e) => ScanResult::Unavailable(e.to_string()),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        ScanResult::Unavailable("virus scanning is only available on Windows".into())
    }
}

/// Shut down Windows now. Only called after the user's cancellable countdown has run out.
pub fn shutdown_windows() -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        std::process::Command::new(Path::new(&sys).join(r"System32\shutdown.exe"))
            .args(["/s", "/t", "0", "/d", "p:0:0", "/c", "Gale: scheduled downloads finished"])
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(windows))]
    {
        tracing::warn!("shutdown requested; ignored on non-Windows builds");
        Ok(())
    }
}
