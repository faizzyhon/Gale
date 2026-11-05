//! App side of the browser bridge. Requests arrive already authenticated (IPC token) and validated.

use gale_core::engine::RequestHeaders;
use gale_core::manager::AddOptions;
use gale_core::protocol::{AddPayload, Command, Envelope, Response};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::{show_main, AppState, PendingAdd};

pub fn headers_for(p: &AddPayload) -> RequestHeaders {
    let mut h = Vec::new();
    if let Some(c) = p.cookies.as_ref().filter(|c| !c.is_empty()) {
        h.push(("Cookie".into(), c.clone()));
    }
    if let Some(r) = p.referrer.as_ref().filter(|r| r.starts_with("http")) {
        h.push(("Referer".into(), r.clone()));
    }
    if let Some(ua) = p.user_agent.as_ref().filter(|u| !u.is_empty()) {
        h.push(("User-Agent".into(), ua.clone()));
    }
    RequestHeaders(h)
}

pub async fn handle(app: AppHandle, env: Envelope) -> Response {
    let st = app.state::<AppState>();
    let id = env.request.id.clone();
    let settings = st.settings();
    let db = st.manager.db();
    match env.request.command {
        Command::Ping => Response::ok(&id, json!({ "version": env!("CARGO_PKG_VERSION"), "enabled": settings.browser_enabled })),
        Command::GetRules => Response::ok(
            &id,
            json!({
                "enabled": settings.browser_enabled,
                "excludedDomains": settings.browser_excluded_domains,
                "excludedTypes": settings.browser_excluded_types,
            }),
        ),
        Command::Recent { limit } => match db.list_downloads().await {
            Ok(list) => Response::ok(
                &id,
                json!(list
                    .iter()
                    .take(limit.clamp(1, 100) as usize)
                    .map(|d| json!({
                        "id": d.id,
                        "fileName": d.file_name,
                        "status": d.status,
                        "progress": d.total_bytes.filter(|t| *t > 0).map(|t| d.downloaded_bytes as f64 / t as f64),
                    }))
                    .collect::<Vec<_>>()),
            ),
            Err(e) => Response::err(&id, "internal", e.to_string()),
        },
        Command::ShowApp => {
            show_main(&app);
            Response::ok(&id, json!({}))
        }
        _ if !settings.browser_enabled => Response::err(&id, "disabled", "Browser integration is turned off in Gale"),
        Command::AddDownload(p) => {
            let _ = db.add_browser_event(if p.silent { "intercepted" } else { "confirm" }, Some(&p.url), p.page_url.as_deref(), None).await;
            if !p.silent {
                // The user confirms in Gale's Add dialog; headers stay here, not in the webview.
                let pending_id = uuid::Uuid::new_v4().to_string();
                let info = json!({ "url": p.url, "fileName": p.filename.as_deref().and_then(|f| f.rsplit(['/', '\\']).next()), "referrer": p.referrer, "fileSize": p.file_size, "mime": p.mime });
                st.pending.lock().unwrap().insert(pending_id.clone(), PendingAdd::new(headers_for(&p), info));
                crate::windows::open_capture(&app, &pending_id);
                return Response::ok(&id, json!({ "pending": true }));
            }
            let queue_id = if p.enqueue { db.default_queue_id().await.ok() } else { None };
            let opts = AddOptions {
                url: p.url.clone(),
                save_dir: settings.download_dir.clone(),
                file_name: p.filename.as_deref().and_then(|f| f.rsplit(['/', '\\']).next()).map(str::to_owned),
                referrer: p.referrer.clone(),
                start: queue_id.is_none(),
                source: Some("browser".into()),
                queue_id,
                headers: headers_for(&p),
                ..Default::default()
            };
            match crate::commands::add_with_policy(&st, opts).await {
                Ok(did) => {
                    crate::windows::want_progress_window(&app, did).await;
                    Response::ok(&id, json!({ "id": did }))
                }
                Err(e) => {
                    let _ = db.add_browser_event("rejected", Some(&p.url), p.page_url.as_deref(), None).await;
                    Response::err(&id, &e.code, e.message)
                }
            }
        }
        Command::AddMedia(m) => {
            let _ = db.add_browser_event("media", Some(&m.url), None, None).await;
            show_main(&app);
            let _ = app.emit("browser://media", json!({ "url": m.url, "title": m.title }));
            Response::ok(&id, json!({ "review": true }))
        }
        Command::ProbeMedia(m) => match st.manager.extract_media(&m.url).await {
            Ok(info) => Response::ok(&id, json!({ "title": info.title, "isLive": info.is_live, "drmOnly": info.drm_only, "options": info.options })),
            Err(e) => {
                let e = crate::commands::CommandError::from(e);
                Response::err(&id, &e.code, e.message)
            }
        },
        Command::DownloadMedia(m) => {
            let _ = db.add_browser_event("media", Some(&m.url), None, None).await;
            let opts = AddOptions {
                url: m.url.clone(),
                save_dir: settings.download_dir.clone(),
                start: true,
                source: Some("browser".into()),
                media: Some(gale_core::manager::MediaRequest { choice: m.choice, kind: m.kind, title: m.title, expected_bytes: m.estimated_size }),
                ..Default::default()
            };
            match crate::commands::add_with_policy(&st, opts).await {
                Ok(did) => {
                    crate::windows::want_progress_window(&app, did).await;
                    Response::ok(&id, json!({ "id": did }))
                }
                Err(e) => Response::err(&id, &e.code, e.message),
            }
        }
        Command::AddBatch(b) => {
            show_main(&app);
            let urls: Vec<String> = b.items.into_iter().map(|i| i.url).collect();
            let _ = app.emit("browser://batch", json!({ "urls": urls, "referrer": b.referrer }));
            Response::ok(&id, json!({ "review": true }))
        }
    }
}
