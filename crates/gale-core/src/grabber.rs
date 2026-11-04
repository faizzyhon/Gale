//! Website grabber: breadth-first crawl that lists downloadable resources.
//! Polite by construction: robots.txt, same-domain by default, delay between pages, hard caps.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::security::{filename_from_url, sanitize_filename, validate_url};
use crate::{Error, Result};

const MAX_PAGE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabOptions {
    pub start_url: String,
    /// 0 = only the start page.
    pub max_depth: u32,
    pub same_domain: bool,
    /// Lowercase extensions without dots, e.g. ["jpg", "pdf"].
    pub extensions: Vec<String>,
    /// Only keep resource URLs containing this text.
    pub include: Option<String>,
    /// Drop resource URLs containing this text.
    pub exclude: Option<String>,
    pub max_pages: usize,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub url: String,
    pub file_name: String,
    pub extension: String,
    pub found_on: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabResult {
    pub resources: Vec<Resource>,
    pub pages_visited: usize,
    pub pages_blocked_by_robots: usize,
}

/// Longest-match robots.txt rules for `*` and `Gale` user agents.
#[derive(Debug, Default)]
struct Robots {
    rules: Vec<(bool, String)>,
}

impl Robots {
    fn parse(txt: &str) -> Self {
        let mut rules = Vec::new();
        let (mut in_group, mut group_has_rules) = (false, false);
        for line in txt.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((k, v)) = line.split_once(':') else { continue };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            match k.as_str() {
                "user-agent" => {
                    if group_has_rules {
                        in_group = false;
                        group_has_rules = false;
                    }
                    let ua = v.to_ascii_lowercase();
                    in_group |= ua == "*" || ua.contains("gale");
                }
                "allow" | "disallow" => {
                    group_has_rules = true;
                    if in_group && !v.is_empty() {
                        rules.push((k == "allow", v.to_string()));
                    }
                }
                _ => {}
            }
        }
        Self { rules }
    }

    fn allows(&self, path: &str) -> bool {
        self.rules
            .iter()
            .filter(|(_, p)| path.starts_with(p.trim_end_matches('*')))
            .max_by_key(|(allow, p)| (p.len(), *allow))
            .is_none_or(|(allow, _)| *allow)
    }
}

pub async fn crawl(client: &Client, o: &GrabOptions, cancel: &CancellationToken) -> Result<GrabResult> {
    let start = validate_url(&o.start_url)?;
    let max_pages = o.max_pages.clamp(1, 500);
    let delay = Duration::from_millis(o.delay_ms.max(200));
    let exts: HashSet<String> = o.extensions.iter().map(|e| e.trim_start_matches('.').to_ascii_lowercase()).collect();
    if exts.is_empty() {
        return Err(Error::InvalidState("choose at least one file type".into()));
    }

    let mut robots: HashMap<String, Robots> = HashMap::new();
    let mut queue = VecDeque::from([(start.clone(), 0u32)]);
    let mut seen_pages = HashSet::from([start.as_str().to_string()]);
    let mut seen_res = HashSet::new();
    let mut out = GrabResult { resources: vec![], pages_visited: 0, pages_blocked_by_robots: 0 };

    while let Some((page, depth)) = queue.pop_front() {
        if cancel.is_cancelled() || out.pages_visited >= max_pages {
            break;
        }
        let origin = page.origin().ascii_serialization();
        if !robots.contains_key(&origin) {
            robots.insert(origin.clone(), fetch_robots(client, &page).await);
        }
        if !robots[&origin].allows(page.path()) {
            out.pages_blocked_by_robots += 1;
            continue;
        }
        if out.pages_visited > 0 {
            tokio::select! { _ = tokio::time::sleep(delay) => {}, _ = cancel.cancelled() => break }
        }
        out.pages_visited += 1;
        let Some(html) = fetch_html(client, &page).await else { continue };
        for link in extract_links(&html, &page) {
            let ext = extension_of(&link);
            if ext.as_deref().is_some_and(|e| exts.contains(e)) {
                let s = link.as_str();
                let keep = o.include.as_deref().is_none_or(|i| i.is_empty() || s.contains(i))
                    && o.exclude.as_deref().is_none_or(|x| x.is_empty() || !s.contains(x));
                if keep && seen_res.insert(s.to_string()) {
                    out.resources.push(Resource {
                        url: s.to_string(),
                        file_name: sanitize_filename(&filename_from_url(&link).unwrap_or_else(|| "download".into())),
                        extension: ext.unwrap_or_default(),
                        found_on: page.to_string(),
                    });
                }
            } else if depth < o.max_depth.min(5)
                && is_page(ext.as_deref())
                && (!o.same_domain || link.host_str() == start.host_str())
                && seen_pages.insert(link.as_str().to_string())
            {
                queue.push_back((link, depth + 1));
            }
        }
    }
    Ok(out)
}

async fn fetch_robots(client: &Client, page: &Url) -> Robots {
    let Ok(u) = page.join("/robots.txt") else { return Robots::default() };
    match client.get(u).send().await {
        Ok(r) if r.status().is_success() => Robots::parse(&r.text().await.unwrap_or_default()),
        Ok(r) if r.status().is_client_error() => Robots::default(), // no robots.txt = everything allowed
        // Server error or unreachable: be conservative and crawl nothing on that host.
        _ => Robots { rules: vec![(false, "/".into())] },
    }
}

async fn fetch_html(client: &Client, page: &Url) -> Option<String> {
    let r = client.get(page.clone()).send().await.ok()?;
    let html = r.status().is_success()
        && r.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|c| c.contains("html"));
    if !html {
        return None;
    }
    let mut body = Vec::new();
    let mut stream = r.bytes_stream();
    while let Some(Ok(chunk)) = stream.next().await {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_PAGE_BYTES {
            break;
        }
    }
    Some(String::from_utf8_lossy(&body).into_owned())
}

fn extract_links(html: &str, base: &Url) -> Vec<Url> {
    // Html is !Send: parse and drop it without crossing an await.
    let doc = Html::parse_document(html);
    let sel = Selector::parse("a[href], img[src], source[src], video[src], audio[src], link[href], embed[src]").expect("static selector");
    doc.select(&sel)
        .filter_map(|e| e.value().attr("href").or_else(|| e.value().attr("src")))
        .filter_map(|h| base.join(h.trim()).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .map(|mut u| {
            u.set_fragment(None);
            u
        })
        .collect()
}

fn extension_of(u: &Url) -> Option<String> {
    let last = u.path_segments()?.next_back()?;
    let (_, ext) = last.rsplit_once('.')?;
    (!ext.is_empty() && ext.len() <= 8).then(|| ext.to_ascii_lowercase())
}

fn is_page(ext: Option<&str>) -> bool {
    matches!(ext, None | Some("html" | "htm" | "php" | "asp" | "aspx" | "jsp" | "shtml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_longest_match() {
        let r = Robots::parse("User-agent: Googlebot\nDisallow: /\n\nUser-agent: *\nDisallow: /private\nAllow: /private/ok\n");
        assert!(r.allows("/"));
        assert!(!r.allows("/private/x"));
        assert!(r.allows("/private/ok/file"));
        assert!(Robots::parse("").allows("/anything"));
    }

    #[test]
    fn links_are_resolved_and_filtered() {
        let base = Url::parse("https://ex.com/dir/page.html").unwrap();
        let links = extract_links(
            r#"<a href="a.pdf#p2">x</a><img src="/img/b.PNG"><a href="mailto:x@y">m</a><a href="javascript:void(0)">j</a>"#,
            &base,
        );
        let s: Vec<_> = links.iter().map(|u| u.as_str()).collect();
        assert_eq!(s, ["https://ex.com/dir/a.pdf", "https://ex.com/img/b.PNG"]);
        assert_eq!(extension_of(&links[1]).as_deref(), Some("png"));
    }
}
