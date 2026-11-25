//! Hugging Face model files: find a model, list its files, group GGUF quantisations and weight formats, and build
//! download links. Every large file comes with its SHA-256 from Hugging Face, which the engine checks after download.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const DEFAULT_ENDPOINT: &str = "https://huggingface.co";
/// Hosts that may receive the user's access token (the official site and the public mirror).
pub const TOKEN_HOSTS: &[&str] = &["huggingface.co", "hf-mirror.com"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub downloads: u64,
    pub likes: u64,
    pub gated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFile {
    pub path: String,
    pub size: u64,
    pub sha256: Option<String>,
    /// Which choice the file belongs to: a GGUF quantisation ("Q4_K_M"), "safetensors", "pytorch", "config", or "other".
    pub group: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    /// "gguf" (pick one), "weights" (pick one), or "extra" (configs and tokenizer, usually all).
    pub kind: &'static str,
    pub size: u64,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub repo: String,
    pub revision: String,
    pub files: Vec<ModelFile>,
    pub groups: Vec<Group>,
    /// Paths selected by default: Q4_K_M (or the closest) for GGUF; otherwise safetensors plus configs.
    pub recommended: Vec<String>,
}

/// `org/name`, or a huggingface.co / hf.co / hf-mirror.com link to the model (any page of it).
pub fn parse_repo(input: &str) -> Option<(String, String)> {
    let t = input.trim().trim_end_matches('/');
    let path = match url::Url::parse(t) {
        Ok(u) if u.host_str().is_some_and(|h| ["huggingface.co", "hf.co", "www.huggingface.co", "hf-mirror.com"].contains(&h)) => {
            u.path().trim_matches('/').to_string()
        }
        Ok(_) => return None,
        Err(_) => t.strip_prefix("hf.co/").or_else(|| t.strip_prefix("huggingface.co/")).unwrap_or(t).to_string(),
    };
    let mut parts = path.split('/').filter(|s| !s.is_empty());
    let (org, name) = (parts.next()?, parts.next()?);
    if ["datasets", "spaces", "docs", "api", "models"].contains(&org) || !valid_part(org) || !valid_part(name) {
        return None;
    }
    // .../tree/<rev>/..., .../blob/<rev>/..., .../resolve/<rev>/...
    let revision = match parts.next() {
        Some("tree" | "blob" | "resolve") => parts.next().unwrap_or("main").to_string(),
        _ => "main".to_string(),
    };
    Some((format!("{org}/{name}"), revision))
}

fn valid_part(s: &str) -> bool {
    !s.is_empty() && s.len() <= 96 && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && s != "." && s != ".."
}

/// The quantisation in a GGUF file name, such as Q4_K_M, IQ3_XS, Q8_0, F16, BF16 or MXFP4.
pub fn quant_of(file: &str) -> Option<String> {
    let name = file.rsplit('/').next()?.to_ascii_uppercase();
    let stem = name.strip_suffix(".GGUF")?;
    // Drop the "-00001-of-00003" part of split files.
    let stem = match stem.rfind("-OF-") {
        Some(i) if stem[i + 4..].chars().all(|c| c.is_ascii_digit()) => stem[..i].rsplit_once('-').map_or(stem, |(a, _)| a),
        _ => stem,
    };
    let tokens: Vec<&str> = stem.split(['-', '.']).collect();
    tokens.iter().rev().find(|t| is_quant(t)).map(|t| t.to_string())
}

fn is_quant(t: &str) -> bool {
    if ["F16", "F32", "BF16", "MXFP4", "FP16", "FP8"].contains(&t) {
        return true;
    }
    let rest = t.strip_prefix("IQ").or_else(|| t.strip_prefix("TQ")).or_else(|| t.strip_prefix('Q'));
    let Some(rest) = rest else { return false };
    let mut chars = rest.chars();
    chars.next().is_some_and(|c| c.is_ascii_digit()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && rest.len() <= 8
}

fn group_of(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    let file = lower.rsplit('/').next().unwrap_or(&lower);
    if lower.ends_with(".gguf") {
        return quant_of(path).unwrap_or_else(|| "GGUF".into());
    }
    let root = !path.contains('/');
    if root && (file.ends_with(".safetensors") || file == "model.safetensors.index.json") {
        return "safetensors".into();
    }
    if root && (file.ends_with(".bin") && file.starts_with("pytorch_model") || file == "pytorch_model.bin.index.json") {
        return "pytorch".into();
    }
    let config = file.ends_with(".json") || file.ends_with(".model") || file.ends_with(".tiktoken") || file.ends_with(".jinja")
        || file == "merges.txt" || file == "vocab.txt";
    if root && config {
        return "config".into();
    }
    "other".into()
}

/// Groups the files and picks the default selection.
pub fn organise(repo: String, revision: String, files: Vec<ModelFile>) -> ModelInfo {
    let mut groups: Vec<Group> = Vec::new();
    for f in &files {
        let kind = match f.group.as_str() {
            "safetensors" | "pytorch" => "weights",
            "config" | "other" => "extra",
            _ => "gguf",
        };
        match groups.iter_mut().find(|g| g.name == f.group) {
            Some(g) => {
                g.size += f.size;
                g.files.push(f.path.clone());
            }
            None => groups.push(Group { name: f.group.clone(), kind, size: f.size, files: vec![f.path.clone()] }),
        }
    }
    groups.sort_by_key(|g| (match g.kind { "gguf" => 0, "weights" => 1, _ => 2 }, g.size));

    let gguf: Vec<&Group> = groups.iter().filter(|g| g.kind == "gguf").collect();
    let recommended = if !gguf.is_empty() {
        // Q4_K_M is the usual balance of size and quality; otherwise the smallest 4-bit, otherwise the smallest.
        let pick = gguf.iter().find(|g| g.name == "Q4_K_M").or_else(|| gguf.iter().find(|g| g.name.starts_with("Q4"))).unwrap_or(&gguf[0]);
        pick.files.clone()
    } else {
        let weights = groups.iter().find(|g| g.name == "safetensors").or_else(|| groups.iter().find(|g| g.name == "pytorch"));
        let config = groups.iter().find(|g| g.name == "config");
        weights.into_iter().chain(config).flat_map(|g| g.files.clone()).collect()
    };
    ModelInfo { repo, revision, files, groups, recommended }
}

fn auth(req: reqwest::RequestBuilder, token: Option<&str>) -> reqwest::RequestBuilder {
    match token {
        Some(t) if !t.is_empty() => req.bearer_auth(t),
        _ => req,
    }
}

fn hf_error(status: reqwest::StatusCode) -> Error {
    match status.as_u16() {
        401 | 403 => Error::InvalidState(
            "This model is gated or private. Accept its terms on huggingface.co, then add your access token in Settings → AI models.".into(),
        ),
        404 => Error::InvalidState("Model not found. Check the name, for example Qwen/Qwen3-8B-GGUF.".into()),
        s => Error::Http { status: s },
    }
}

#[derive(Deserialize)]
struct ApiModel {
    id: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    likes: u64,
    #[serde(default)]
    gated: serde_json::Value,
}

pub async fn search(client: &Client, endpoint: &str, query: &str, token: Option<&str>) -> Result<Vec<SearchHit>> {
    let url = url::Url::parse_with_params(
        &format!("{endpoint}/api/models"),
        &[("search", query), ("limit", "25"), ("sort", "downloads"), ("direction", "-1")],
    )
    .map_err(|e| Error::InvalidUrl(e.to_string()))?;
    let resp = auth(client.get(url), token).send().await?;
    if !resp.status().is_success() {
        return Err(hf_error(resp.status()));
    }
    let models: Vec<ApiModel> = serde_json::from_slice(&resp.bytes().await?).map_err(|e| Error::InvalidState(e.to_string()))?;
    Ok(models
        .into_iter()
        .map(|m| SearchHit { id: m.id, downloads: m.downloads, likes: m.likes, gated: !matches!(m.gated, serde_json::Value::Bool(false) | serde_json::Value::Null) })
        .collect())
}

#[derive(Deserialize)]
struct TreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    #[serde(default)]
    size: u64,
    lfs: Option<Lfs>,
}

#[derive(Deserialize)]
struct Lfs {
    oid: String,
}

pub async fn list(client: &Client, endpoint: &str, repo: &str, revision: &str, token: Option<&str>) -> Result<ModelInfo> {
    let mut next = Some(format!("{endpoint}/api/models/{repo}/tree/{revision}?recursive=true"));
    let mut files = Vec::new();
    while let Some(url) = next.take() {
        let resp = auth(client.get(&url), token).send().await?;
        if !resp.status().is_success() {
            return Err(hf_error(resp.status()));
        }
        // Large repositories come in pages: Link: <...>; rel="next"
        next = resp
            .headers()
            .get(reqwest::header::LINK)
            .and_then(|v| v.to_str().ok())
            .and_then(|l| l.split(',').find(|p| p.contains("rel=\"next\"")))
            .and_then(|p| Some(p.split_once('<')?.1.split_once('>')?.0.to_string()))
            .filter(|u| u.starts_with(endpoint));
        let entries: Vec<TreeEntry> = serde_json::from_slice(&resp.bytes().await?).map_err(|e| Error::InvalidState(e.to_string()))?;
        for e in entries.into_iter().filter(|e| e.kind == "file") {
            let sha256 = e.lfs.map(|l| l.oid).filter(|o| o.len() == 64 && o.bytes().all(|b| b.is_ascii_hexdigit()));
            files.push(ModelFile { group: group_of(&e.path), path: e.path, size: e.size, sha256 });
        }
    }
    if files.is_empty() {
        return Err(Error::InvalidState("This model has no files.".into()));
    }
    Ok(organise(repo.to_string(), revision.to_string(), files))
}

/// The address of one file. Each path part is encoded; Hugging Face redirects large files to its CDN.
pub fn file_url(endpoint: &str, repo: &str, revision: &str, path: &str) -> String {
    let enc = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>().replace('+', "%20");
    let path: Vec<String> = path.split('/').map(enc).collect();
    format!("{endpoint}/{repo}/resolve/{}/{}", enc(revision), path.join("/"))
}

/// Where LM Studio keeps models (`~/.lmstudio/models`, or the older `~/.cache/lm-studio/models`), if installed.
pub fn lm_studio_dir() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    [home.join(".lmstudio").join("models"), home.join(".cache").join("lm-studio").join("models")].into_iter().find(|p| p.is_dir())
}

/// A file path from the repository, checked so it cannot leave the model folder.
pub fn safe_relative(path: &str) -> Option<std::path::PathBuf> {
    let mut out = std::path::PathBuf::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            return None;
        }
        out.push(crate::security::sanitize_filename(part));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_from_names_and_links() {
        let main = |r: &str| Some((r.to_string(), "main".to_string()));
        assert_eq!(parse_repo("Qwen/Qwen3-8B-GGUF"), main("Qwen/Qwen3-8B-GGUF"));
        assert_eq!(parse_repo("https://huggingface.co/unsloth/gemma-3-4b-it-GGUF/"), main("unsloth/gemma-3-4b-it-GGUF"));
        assert_eq!(parse_repo("hf.co/bartowski/Llama-3.2-1B-Instruct-GGUF"), main("bartowski/Llama-3.2-1B-Instruct-GGUF"));
        assert_eq!(
            parse_repo("https://huggingface.co/org/m/blob/v2/sub/model.gguf"),
            Some(("org/m".into(), "v2".into()))
        );
        assert_eq!(parse_repo("https://example.com/org/m"), None);
        assert_eq!(parse_repo("https://huggingface.co/datasets/x/y"), None);
        assert_eq!(parse_repo("just-one-word"), None);
        assert_eq!(parse_repo("org/../etc"), None);
    }

    #[test]
    fn quantisations_from_file_names() {
        assert_eq!(quant_of("Qwen3-8B-Q4_K_M.gguf").as_deref(), Some("Q4_K_M"));
        assert_eq!(quant_of("sub/gemma-3-4b-it-IQ3_XS.gguf").as_deref(), Some("IQ3_XS"));
        assert_eq!(quant_of("model.Q8_0.gguf").as_deref(), Some("Q8_0"));
        assert_eq!(quant_of("Llama-70B-Q4_K_M-00002-of-00003.gguf").as_deref(), Some("Q4_K_M"));
        assert_eq!(quant_of("phi-4-BF16.gguf").as_deref(), Some("BF16"));
        assert_eq!(quant_of("gpt-oss-20b-MXFP4.gguf").as_deref(), Some("MXFP4"));
        assert_eq!(quant_of("Qwen2-7B.gguf"), None);
        assert_eq!(quant_of("config.json"), None);
    }

    fn f(path: &str, size: u64) -> ModelFile {
        ModelFile { path: path.into(), size, sha256: None, group: group_of(path) }
    }

    #[test]
    fn gguf_repo_picks_q4_k_m_including_all_its_parts() {
        let info = organise(
            "o/m".into(),
            "main".into(),
            vec![
                f("README.md", 5),
                f("m-Q8_0.gguf", 800),
                f("m-Q4_K_M-00001-of-00002.gguf", 250),
                f("m-Q4_K_M-00002-of-00002.gguf", 250),
                f("m-Q2_K.gguf", 300),
            ],
        );
        assert_eq!(info.recommended, vec!["m-Q4_K_M-00001-of-00002.gguf", "m-Q4_K_M-00002-of-00002.gguf"]);
        let q4 = info.groups.iter().find(|g| g.name == "Q4_K_M").unwrap();
        assert_eq!(q4.size, 500);
        assert_eq!(info.groups[0].kind, "gguf");
        assert_eq!(info.groups.last().unwrap().name, "other");
    }

    #[test]
    fn weights_repo_picks_safetensors_and_configs_not_other_formats() {
        let info = organise(
            "o/m".into(),
            "main".into(),
            vec![
                f("config.json", 1),
                f("tokenizer.json", 2),
                f("model-00001-of-00002.safetensors", 100),
                f("model-00002-of-00002.safetensors", 100),
                f("model.safetensors.index.json", 1),
                f("pytorch_model.bin", 300),
                f("original/consolidated.00.pth", 300),
                f("onnx/model.onnx", 50),
            ],
        );
        let mut r = info.recommended.clone();
        r.sort();
        assert_eq!(
            r,
            vec!["config.json", "model-00001-of-00002.safetensors", "model-00002-of-00002.safetensors", "model.safetensors.index.json", "tokenizer.json"]
        );
    }

    #[test]
    fn file_links_and_safe_paths() {
        assert_eq!(file_url(DEFAULT_ENDPOINT, "o/m", "main", "sub dir/a+b.gguf"), "https://huggingface.co/o/m/resolve/main/sub%20dir/a%2Bb.gguf");
        assert_eq!(safe_relative("onnx/model.onnx"), Some(std::path::PathBuf::from("onnx").join("model.onnx")));
        assert_eq!(safe_relative("../x"), None);
        assert_eq!(safe_relative("a//b"), None);
        assert_eq!(safe_relative("C:/x"), None);
    }
}
