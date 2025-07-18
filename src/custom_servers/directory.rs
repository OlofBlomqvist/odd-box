
#![allow(clippy::needless_pass_by_value)]

use axum::http::HeaderValue;
use bytes::Bytes;
use dashmap::DashMap;
use httpdate::{fmt_http_date, parse_http_date};
use hyper::{
    header::{CONTENT_TYPE, COOKIE, IF_MODIFIED_SINCE, IF_NONE_MATCH, VARY},
    Response, StatusCode,
};
use mime_guess::mime;
use once_cell::sync::Lazy;
use std::{
    f32::consts::E, path::{Path, PathBuf}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}
};


use crate::{
    configuration::DirServer,
    http_proxy::{create_simple_response_from_bytes, EpicResponse},
    CustomError,
};

/// `(content_type, inserted_at, full_response)`
pub type CacheValue = (String, Instant, Response<Bytes>);

pub static RESPONSE_CACHE: Lazy<DashMap<String, CacheValue>> = Lazy::new(DashMap::new);

/// User‑theme preference derived from the `theme` cookie.
#[derive(Clone, Copy)]
pub enum ThemeDecision {
    Dark,
    Light,
    Auto,
}

impl ThemeDecision {
    fn from_req(req: &hyper::Request<hyper::body::Incoming>) -> Self {
        req.headers()
            .get(COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|raw| {
                raw.split(';')
                    .find_map(|pair| cookie::Cookie::parse(pair.trim()).ok())
            })
            .filter(|c| c.name() == "theme")
            .map(|c| match c.value().trim() {
                "dark" => ThemeDecision::Dark,
                "light" => ThemeDecision::Light,
                _ => ThemeDecision::Auto,
            })
            .unwrap_or(ThemeDecision::Auto)
    }
}

pub async fn handle(
    target: DirServer,
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<EpicResponse, CustomError> {
    

    
    let path_decoded: String = urlencoding::decode(req.uri().path())
        .map_err(|e| CustomError(format!("{e:?}")))?
        .into_owned(); 

    let cookie_sig = req
        .headers()
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("no-cookies");

    let cache_key = format!(
        "{}-{}-{}",
        target.host_name,
        cookie_sig,
        &path_decoded.trim_end_matches('/')
    );
    tracing::trace!(%cache_key, "incoming request");

    // ----------------------- fast‑path in‑memory cache ----------------------
    if let Some(resp) = cached_fresh_response(&cache_key)? {
        return create_simple_response_from_bytes(resp);
    }

    // ----------------- canonicalize path & traversal defence ---------------
   
    let target = target.clone();
    let path_decoded_clone = path_decoded.clone();
    let path = PathBuf::from(&target.dir).join(
        Path::new(&*path_decoded_clone)
            .strip_prefix("/")
            .unwrap_or(Path::new(&*path_decoded_clone)),
    );

    let full_path = match tokio::fs::canonicalize(&path).await {
        Ok(v) => v,
        Err(e) => {
            match e.kind() {
                std::io::ErrorKind::NotFound => return simple_status(StatusCode::NOT_FOUND, "404 - FILE NOT FOUND"),
                _ => return Err(map_io(e, "canonicalize")),
            }
        },
    };

    if !full_path.exists() {
        return simple_status(StatusCode::NOT_FOUND, "404 - NOT FOUND");
    }

    

    if !full_path.starts_with(&target.dir) {
        return Err(CustomError("Attempted directory traversal".into()));
    }

    if full_path.is_file() {
        serve_file(&target, &req, &full_path, cache_key).await
    } else if full_path.is_dir() {
        serve_directory(&target, &req, &full_path, &path_decoded, cache_key).await
    } else {
        simple_status(StatusCode::NOT_FOUND, "Not Found")
    }
}

// ==========================================================================
// Caching helpers
// ==========================================================================

fn cached_fresh_response(key: &str) -> Result<Option<Response<Bytes>>,CustomError> {
    // acquire read-guard
    if let Some(entry) = RESPONSE_CACHE.get(key) {
        let (_, inserted, resp) = entry.value();
        if inserted.elapsed() < Duration::from_secs(10) {
            // fresh — clone while guard is alive
            let mut cloned = resp.clone();
            cloned
                .headers_mut()
                .insert("mem-cached", HeaderValue::from_static("true"));
            cloned
                .headers_mut()
                .insert("cache-key", HeaderValue::from_str(key).map_err(|e|CustomError(format!("{e:?}")))?);
            return Ok(Some(cloned));
        }
        // -------- stale path ---------
        drop(entry);                  // <-- RELEASE read-lock first
        RESPONSE_CACHE.remove(key);   // safe: now we can take the write-lock
    }
    Ok(None)
}

/// Insert a fresh response into the shared cache.
fn cache_response(key: String, content_type: &str, resp: &Response<Bytes>) {
    RESPONSE_CACHE.insert(
        key,
        (content_type.to_owned(), Instant::now(), resp.clone()),
    );
}

// ==========================================================================
// File‑serving helpers
// ==========================================================================

async fn serve_file(
    cfg: &DirServer,
    req: &hyper::Request<hyper::body::Incoming>,
    full_path: &Path,
    cache_key: String,
) -> Result<EpicResponse, CustomError> {

    let Some((meta, file_bytes)) = read_file(full_path).await? else {
        return simple_status(StatusCode::NOT_FOUND, "404 - FILE NOT FOUND")
    };
    let last_modified = fmt_http_date(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
    let size = meta.len();
    let etag = format!(
        "\"{}-{}-{cache_key}\"",
        size,
        meta.modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    // ------------ conditional GET ----------------------------------------
    if is_not_modified(req, &etag, meta.modified().ok())? {
        return simple_status(StatusCode::NOT_MODIFIED, "");
    }

    // ------------ Markdown branch ----------------------------------------
    let ext_is_md = full_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("md"))
        .unwrap_or(false);

    if cfg.render_markdown.unwrap_or_default() && ext_is_md {
        return serve_markdown(
            cfg,
            &file_bytes,
            &etag,
            &last_modified,
            &cache_key,
            ThemeDecision::from_req(req),
        )
        .await;
    }

    // ------------ Binary / plain‑text branch -----------------------------
    let mut mime_type = mime_guess::from_path(full_path).first_or_octet_stream();
    if ext_is_md {
        mime_type = mime::TEXT_PLAIN_UTF_8;
    }

    let resp = build_response(
        StatusCode::OK,
        mime_type.as_ref(),
        cfg.cache_control_max_age_in_seconds,
        Some((&etag, &last_modified)),
        file_bytes.clone(),
    )?;
    cache_response(cache_key, mime_type.as_ref(), &resp);
    create_simple_response_from_bytes(resp)
}

async fn serve_markdown(
    cfg: &DirServer,
    md_bytes: &Bytes,
    etag: &str,
    last_modified: &str,
    cache_key: &str,
    theme: ThemeDecision,
) -> Result<EpicResponse, CustomError> {

    let md = String::from_utf8_lossy(md_bytes).into_owned();

    let html = super::markdown_to_html(theme, "markdown", &md)
        .map_err(|e| CustomError(format!("Markdown→HTML failed: {e}").into()))?;

    let body = Bytes::from(html.into_bytes());
    let resp = build_response(
        StatusCode::OK,
        "text/html; charset=utf-8",
        cfg.cache_control_max_age_in_seconds,
        Some((etag, last_modified)),
        body.clone(),
    )?;
    cache_response(cache_key.to_string(), "text/html; charset=utf-8", &resp);
    create_simple_response_from_bytes(resp)
}

// ==========================================================================
// Directory‑serving helpers
// ==========================================================================

async fn serve_directory(
    cfg: &DirServer,
    req: &hyper::Request<hyper::body::Incoming>,
    dir: &Path,
    req_path: &str,
    cache_key: String,
) -> Result<EpicResponse, CustomError> {

    // Redirect “/foo” → “/foo/”
    if !req_path.ends_with('/') {
        let resp = Response::builder()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header("Location", format!("{}/", req_path))
            .body(Bytes::new())
            .map_err(|e|CustomError(format!("{e:?}")))?;

        return create_simple_response_from_bytes(resp);
    }

    // --------------- default file (index.*) ------------------------------
    for name in ["index.html", "index.htm", "index.md"] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return serve_file(cfg, req, &candidate, cache_key).await;
        }
    }

    // --------------- directory listing -----------------------------------
    if !cfg.enable_directory_browsing.unwrap_or_default() {
        return simple_status(StatusCode::FORBIDDEN, "Directory browsing is disabled");
    }

    let dir = dir.to_owned();
    let req_path = req_path.to_owned();
    let listing_html = build_dir_listing(&dir, &req_path)?;

    let resp = build_response(
        StatusCode::OK,
        "text/html; charset=utf-8",
        cfg.cache_control_max_age_in_seconds,
        None,
        Bytes::from(listing_html.into_bytes()),
    )?;
    cache_response(cache_key, "text/html; charset=utf-8", &resp);
    create_simple_response_from_bytes(resp)
}

// ==========================================================================
// Tiny helpers
// ==========================================================================

fn is_not_modified(
    req: &hyper::Request<hyper::body::Incoming>,
    etag: &str,
    modified: Option<SystemTime>,
) -> Result<bool, CustomError> {
    if let Some(hv) = req.headers().get(IF_NONE_MATCH) {
        if hv.to_str().unwrap_or("") == etag {
            return Ok(true);
        }
    }
    if let (Some(modified), Some(hv)) = (modified, req.headers().get(IF_MODIFIED_SINCE)) {
        if let Ok(since) = parse_http_date(hv.to_str().unwrap_or("")) {
            if modified <= since {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

async fn read_file(path: &Path) -> Result<Option<(std::fs::Metadata, Bytes)>, CustomError> {
    match tokio::fs::metadata(path).await {
        Ok(meta) => {
            match tokio::fs::read(path).await {
                Ok(bytes) => Ok(Some((meta, Bytes::from(bytes)))),
                Err(e) => match e.kind() {
                    std::io::ErrorKind::NotFound => Ok(None),
                    _ => {
                        Err(map_io(e, "read"))
                    },
                }
            }
        }
        Err(_) => Ok(None), 
    }
}

fn build_response(
    status: StatusCode,
    content_type: &str,
    max_age: Option<u64>,
    conditional: Option<(&str, &str)>,
    body: Bytes,
) -> Result<Response<Bytes>, CustomError> {
    let mut builder = Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("Vary", HeaderValue::from_static("Cookie"));

    if let Some(max_age) = max_age {
        builder = builder.header(
            "Cache-Control",
            format!("public, max-age={max_age}, immutable"),
        );
    }
    if let Some((etag, last_modified)) = conditional {
        builder = builder.header("ETag", etag).header("Last-Modified", last_modified);
    }
    builder
        .body(body.into())
        .map_err(|e| CustomError(format!("failed to build response: {e}").into()))
}

fn simple_status(code: StatusCode, body: &str) -> Result<EpicResponse, CustomError> {
    create_simple_response_from_bytes(
        Response::builder()
            .status(code)
            .header(CONTENT_TYPE, "text/plain")
            .header(VARY,"Cookie")
            .body(Bytes::from(body.to_owned()))
            .map_err(|e|CustomError(format!("{e:?}")))?
    )
}

fn map_io(e: std::io::Error, ctx: &str) -> CustomError {
    CustomError(format!("io::{ctx} failed: {e}").into())
}

// ==========================================================================
// Directory‑listing renderer
// ==========================================================================

fn build_dir_listing(dir: &Path, req_path: &str) -> Result<String, CustomError> {
    use std::collections::HashMap;

    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| map_io(e, "read_dir"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| map_io(e, "read_dir entry"))?;

    entries.sort_by(|a, b| {
        let a_is_dir = a.metadata().map(|m| m.is_dir()).unwrap_or(false);
        let b_is_dir = b.metadata().map(|m| m.is_dir()).unwrap_or(false);
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&b.file_name().to_string_lossy().to_lowercase()),
        }
    });

    fn icon(ext: &str) -> &'static str {
        static ICONS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
            [
                ("png", "🖼️"),
                ("jpg", "🖼️"),
                ("jpeg", "🖼️"),
                ("gif", "🖼️"),
                ("bmp", "🖼️"),
                ("txt", "📄"),
                ("md", "📝"),
                ("html", "🌐"),
                ("css", "🎨"),
                ("js", "📜"),
                ("json", "📄"),
                ("rs", "🦀"),
                ("py", "🐍"),
                ("java", "☕"),
                ("c", "📘"),
                ("cpp", "📘"),
                ("cs", "💻"),
                ("pdf", "📕"),
                ("zip", "📦"),
                ("tar", "📦"),
                ("gz", "📦"),
                ("mp3", "🎵"),
                ("mp4", "🎥"),
                ("wav", "🎶"),
                ("doc", "📄"),
                ("docx", "📄"),
                ("xls", "📊"),
                ("xlsx", "📊"),
            ]
            .into_iter()
            .collect()
        });
        ICONS.get(ext).copied().unwrap_or("📄")
    }

    fn fmt_size(size: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut sz = size as f64;
        let mut idx = 0;
        while sz >= 1024.0 && idx < UNITS.len() - 1 {
            sz /= 1024.0;
            idx += 1;
        }
        format!("{:.2} {}", sz, UNITS[idx])
    }

    let mut out = String::from(
        r#"<!doctype html><meta charset=utf-8><title>Directory listing</title>
<style>
body{font-family:Arial,system-ui;margin:0;padding:2rem;background:#161a29;color:#fff}
a{color:#1e90ff;text-decoration:none}
a:hover{text-decoration:underline}
ul{list-style:none;padding:0;max-width:900px;margin:auto;border:4px solid transparent;
background-image:linear-gradient(45deg,#ff0000,#ff7f00,#ffff00,#00ff00,#0000ff,#4b0082,#8f00ff,#ff0000);
background-size:200% 200%;animation:rb 10s alternate-reverse infinite;border-radius:8px}
li{display:flex;padding:.6rem 1rem;border-bottom:1px solid #333;background:#1e2336}
li:nth-child(even){background:#0e1623}.icon{margin-right:.6rem}.size{margin-left:auto;color:#888}
@keyframes rb{0%{background-position:0 50%}100%{background-position:100% 50%}}
</style><ul>"#,
    );

    // “up” link
    if req_path != "/" {
        let parent = Path::new(req_path)
            .parent()
            .unwrap_or_else(|| Path::new("/"))
            .to_string_lossy();
        out.push_str(&format!(
            r#"<li><span class=icon>📁</span><a href="{parent}">.. (parent)</a></li>"#
        ));
    }

    for e in entries {
        let meta = e.metadata().map_err(|e| map_io(e, "metadata"))?;
        let name = e.file_name().to_string_lossy().into_owned();
        let href = format!("{req_path}{}", name);
        if meta.is_dir() {
            out.push_str(&format!(
                r#"<li><span class=icon>📁</span><a href="{href}/">{name}/</a></li>"#
            ));
        } else {
            let ext = name.rsplit('.').next().unwrap_or("");
            out.push_str(&format!(
                r#"<li><span class=icon>{}</span><a href="{href}">{name}</a><span class=size>({})</span></li>"#,
                icon(ext),
                fmt_size(meta.len())
            ));
        }
    }
    out.push_str("</ul>");
    Ok(out)
}
