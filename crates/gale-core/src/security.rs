//! Filename and path hardening. Everything that turns remote input into a local path goes through here.

use std::path::{Path, PathBuf};

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];
/// Leaves room for " (999)" and ".gale-part" inside Windows' 255-unit component limit.
const MAX_NAME_UNITS: usize = 200;

/// Turn an untrusted name (header, URL, extension message) into a safe single path component.
pub fn sanitize_filename(raw: &str) -> String {
    // Keep only the last component so "../../x" or "C:\\x" can never escape.
    let last = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let mut name: String = last
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            // Bidi overrides are used to disguise "exe" as e.g. "gpj.exe".
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' => '_',
            c => c,
        })
        .collect();
    name = name.trim().trim_end_matches(['.', ' ']).to_string();
    if name.is_empty() || name == "." || name == ".." {
        name = "download".into();
    }
    let stem_upper = name.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    if RESERVED.contains(&stem_upper.as_str()) {
        name = format!("_{name}");
    }
    truncate_utf16(&name, MAX_NAME_UNITS)
}

fn truncate_utf16(name: &str, max: usize) -> String {
    if name.encode_utf16().count() <= max {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if name.len() - i <= 16 && i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let budget = max - ext.encode_utf16().count();
    let mut out = String::new();
    let mut used = 0;
    for c in stem.chars() {
        used += c.len_utf16();
        if used > budget {
            break;
        }
        out.push(c);
    }
    out + ext
}

/// Join a sanitised name onto a trusted directory, refusing anything that would land outside it.
pub fn safe_join(dir: &Path, name: &str) -> Result<PathBuf, crate::Error> {
    let clean = sanitize_filename(name);
    let joined = dir.join(&clean);
    if joined.parent() != Some(dir) {
        return Err(crate::Error::UnsafePath(name.to_string()));
    }
    Ok(joined)
}

/// `file.zip` -> `file (1).zip` … until a free name is found (also checks the in-progress temp name).
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let taken = |p: &Path| p.exists() || temp_path_for(p).exists();
    let first = dir.join(name);
    if !taken(&first) {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !taken(p))
        .expect("unbounded iterator")
}

pub fn temp_path_for(final_path: &Path) -> PathBuf {
    let mut s = final_path.as_os_str().to_owned();
    s.push(".gale-part");
    PathBuf::from(s)
}

/// Extract a filename from a Content-Disposition header (RFC 6266, incl. `filename*=UTF-8''…`).
pub fn filename_from_content_disposition(header: &str) -> Option<String> {
    let mut plain = None;
    for part in header.split(';').map(str::trim) {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k.trim().to_ascii_lowercase().as_str() {
            "filename*" => {
                let v = v.trim().trim_matches('"');
                let encoded = v.splitn(3, '\'').nth(2).unwrap_or(v);
                if let Ok(s) = percent_encoding::percent_decode_str(encoded).decode_utf8() {
                    if !s.is_empty() {
                        return Some(s.into_owned());
                    }
                }
            }
            "filename" => plain = Some(v.trim().trim_matches('"').to_string()),
            _ => {}
        }
    }
    plain.filter(|s| !s.is_empty())
}

pub fn filename_from_url(url: &url::Url) -> Option<String> {
    let seg = url.path_segments()?.rev().find(|s| !s.is_empty())?;
    let decoded = percent_encoding::percent_decode_str(seg).decode_utf8().ok()?;
    Some(decoded.into_owned())
}

/// Only schemes the engine can actually fetch are accepted from any untrusted source.
pub fn validate_url(input: &str) -> Result<url::Url, crate::Error> {
    let url = url::Url::parse(input.trim()).map_err(|_| crate::Error::InvalidUrl(input.to_string()))?;
    match url.scheme() {
        "http" | "https" if url.host().is_some() => Ok(url),
        _ => Err(crate::Error::InvalidUrl(input.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_traversal_and_reserved_chars() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("..\\..\\Windows\\win.ini"), "win.ini");
        assert_eq!(sanitize_filename("a<b>c:d\"e|f?g*h.txt"), "a_b_c_d_e_f_g_h.txt");
        assert_eq!(sanitize_filename("report.pdf.  . "), "report.pdf");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename(""), "download");
    }

    #[test]
    fn reserved_device_names_are_prefixed() {
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("nul.txt"), "_nul.txt");
        assert_eq!(sanitize_filename("console.txt"), "console.txt");
    }

    #[test]
    fn bidi_override_is_neutralised() {
        assert_eq!(sanitize_filename("invoice\u{202E}fdp.exe"), "invoice_fdp.exe");
    }

    #[test]
    fn long_names_keep_extension() {
        let n = sanitize_filename(&format!("{}.zip", "a".repeat(500)));
        assert!(n.ends_with(".zip"));
        assert_eq!(n.encode_utf16().count(), MAX_NAME_UNITS);
    }

    #[test]
    fn safe_join_stays_in_dir() {
        let dir = Path::new("/downloads");
        assert_eq!(safe_join(dir, "../x.bin").unwrap(), dir.join("x.bin"));
    }

    #[test]
    fn content_disposition_variants() {
        assert_eq!(filename_from_content_disposition("attachment; filename=\"a b.zip\"").as_deref(), Some("a b.zip"));
        assert_eq!(
            filename_from_content_disposition("attachment; filename=x.zip; filename*=UTF-8''%E2%82%AC%20rates.pdf").as_deref(),
            Some("€ rates.pdf")
        );
        assert_eq!(filename_from_content_disposition("inline"), None);
    }

    #[test]
    fn url_validation() {
        assert!(validate_url("https://example.com/f.zip").is_ok());
        assert!(validate_url("file:///C:/Windows/system.ini").is_err());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("not a url").is_err());
        let u = validate_url("https://h/a/My%20File.iso?x=1").unwrap();
        assert_eq!(filename_from_url(&u).as_deref(), Some("My File.iso"));
    }

    #[test]
    fn unique_path_increments() {
        let d = tempfile_dir();
        std::fs::write(d.join("f.txt"), b"").unwrap();
        std::fs::write(d.join("f (1).txt.gale-part"), b"").unwrap();
        assert_eq!(unique_path(&d, "f.txt"), d.join("f (2).txt"));
        std::fs::remove_dir_all(d).ok();
    }

    fn tempfile_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("gale-sec-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
