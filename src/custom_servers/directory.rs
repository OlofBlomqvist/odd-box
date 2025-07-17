use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use axum::http::HeaderValue;
use hyper::{
    header::{COOKIE, IF_MODIFIED_SINCE, IF_NONE_MATCH}, Response
};
use mime_guess::mime;
use httpdate::{fmt_http_date, parse_http_date};
use crate::{
    configuration::DirServer,
    http_proxy::{create_simple_response_from_bytes, EpicResponse},
    CustomError,
};
use once_cell::sync::Lazy;
pub type CacheValue = (String, Instant, Response<bytes::Bytes>); // (Content-Type, Cache Time, Response)
use dashmap::DashMap;


pub enum ThemeDecision {
    Dark,
    Light,
    Auto,
}

// TODO: This cache is global and shared across all requests for the same url
// and we clear it every 30 seconds. Normally we only use values younger than 10 seconds tho.
// While this helps with performance, it can lead to stale data being served incorrectly
// as we don't care about the incoming requests preference.
// At some point we might want to implement a more sophisticated cache
pub static RESPONSE_CACHE: Lazy<DashMap<String, CacheValue>> = Lazy::new(|| {
    DashMap::new()
});

/// Return `true` if the user has a cookie `theme=dark`.
fn req_is_dark(req: &hyper::Request<hyper::body::Incoming>) -> ThemeDecision {
    match req.headers().get(COOKIE) {
        Some(cookie_header) => {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for cookie_str in cookie_str.split(';') {
                    if let Ok(cookie) = cookie::Cookie::parse(cookie_str.trim()) {
                        if cookie.name() == "theme" {
                            return if cookie.value().to_string().trim() == "dark" {
                                ThemeDecision::Dark
                            } else {
                                ThemeDecision::Light
                            };
                        }
                    }
                }
            }
        }
        None => {}
    }
    ThemeDecision::Auto
}

pub async fn handle(
    target: DirServer,
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<EpicResponse, CustomError> {

    
    use mime_guess::from_path;

    let root_dir = Path::new(&target.dir);
    let req_path: String =
        urlencoding::decode(req.uri().path()).map_err(|e| CustomError(format!("{e:?}")))?.to_string();

    let cache_key = req_path.trim_end_matches('/').to_string();
    let c = if let Some(c) = req.headers().get(COOKIE) {
        c.to_str().unwrap_or_default()
    } else { 
        "no-cookies"
    };

    let cache_key = format!("{}-{}-{}", target.host_name, c, cache_key);
    tracing::warn!("Cache key: {}", cache_key);
    
    {
        let mut expired_in_cache = false;
        if let Some(guard) = RESPONSE_CACHE.get(&cache_key) {
            let (_content_type, cache_time, res) = guard.value();
            if cache_time.elapsed() < Duration::from_secs(10) {
                let response = create_simple_response_from_bytes(res.clone());
                match response {
                    Ok(mut resp) => {
                        resp.headers_mut().insert("Vary", 
                        HeaderValue::from(COOKIE)
                        );
                        resp.headers_mut().insert("cache-key", HeaderValue::from_str(&cache_key).unwrap());
                        resp.headers_mut().insert("mem-cached", HeaderValue::from_static("true"));
                        return Ok(resp);
                    },
                    Err(e) => {
                        tracing::trace!("Ignoring bad response from cache: {}", e);
                    }
                } 
            } else {
                expired_in_cache = true;
            }
        }
        if expired_in_cache {
            RESPONSE_CACHE.remove(&cache_key);
        }
    }

    let requested_path = Path::new(&req_path);
    let full_path = root_dir.join(requested_path.strip_prefix("/").unwrap_or(requested_path));
    let full_path = match fs::canonicalize(&full_path) {
        Ok(path) => path,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                let resp = Response::builder()
                    .status(404)
                    .header("Content-Type", "text/plain")
                    .body("sorry, there is nothing here..".into())
                    .unwrap();
                return create_simple_response_from_bytes(resp);
            }
            return Err(CustomError(format!("Failed to canonicalize path: {}", e).into()));
        }
    };
    if !full_path.starts_with(root_dir) {
        return Err(CustomError("Attempted directory traversal".into()));
    }

    // --- FILE BRANCH WITH CONDITIONAL GET & HEADERS ---   
    if full_path.is_file() {
        // 1) Gather metadata for ETag / Last-Modified
        let meta = fs::metadata(&full_path)
            .map_err(|e| CustomError(format!("Failed to stat file: {}", e).into()))?;
        let modified: SystemTime = meta.modified()
            .map_err(|e| CustomError(format!("Failed to get mtime: {}", e).into()))?;
        let last_modified = fmt_http_date(modified);
        let size = meta.len();
        let etag = format!("\"{}-{cache_key}-{}\"", size, modified.duration_since(UNIX_EPOCH).unwrap().as_secs());

        let maybe_cache_max_age = target.cache_control_max_age_in_seconds;
        // 2) Handle If-None-Match → 304
        if let Some(hv) = req.headers().get(IF_NONE_MATCH) {
            if hv.to_str().unwrap_or("") == etag {
            let mut builder = Response::builder().status(304)
                .header("ETag", &etag)
                .header("Last-Modified", &last_modified)
                .header("Vary", 
                        HeaderValue::from(COOKIE)
                )
                .header("cache-key", HeaderValue::from_str(&cache_key).unwrap());
               
            if let Some(max_age) = maybe_cache_max_age {
                let cache_control_header = format!("public, max-age={}, immutable", max_age);
                builder = builder
                    .header("Cache-Control", cache_control_header);
            }
            let resp304 = builder
                .body(bytes::Bytes::new().into())
                .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;
            return create_simple_response_from_bytes(resp304);
            }
        }
        // 3) Handle If-Modified-Since → 304
        if let Some(hv) = req.headers().get(IF_MODIFIED_SINCE) {
            if let Ok(since) = parse_http_date(hv.to_str().unwrap_or("")) {
            if modified <= since {
                let mut builder = Response::builder().status(304)
                .header("ETag", &etag)
                .header("Last-Modified", &last_modified)
                .header("Vary", 
                        HeaderValue::from(COOKIE)
                        );
                if let Some(max_age) = maybe_cache_max_age {
                let cache_control_header = format!("public, max-age={}, immutable", max_age);
                builder = builder.header("Cache-Control", cache_control_header);
                builder = builder.header("cache-key", HeaderValue::from_str(&cache_key).unwrap());
                }
                let resp304 = builder
                .body(bytes::Bytes::new().into())
                .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;
                return create_simple_response_from_bytes(resp304);
            }
            }
        }

        // 4) Read & serve
        let file_content = fs::read(&full_path)
            .map_err(|e| CustomError(format!("Failed to read file: {}", e).into()))?;

        // Markdown rendering branch
        if target.render_markdown.unwrap_or_default()
            && full_path.extension().and_then(|e| e.to_str()).map(|v| v.eq_ignore_ascii_case("md")).unwrap_or(false)
        {
            let markdown = String::from_utf8_lossy(&file_content);
            let html = super::markdown_to_html(
                req_is_dark(&req),
                full_path.file_name().unwrap().to_str().unwrap_or("unnamed"),
                &markdown,
            ).map_err(|e| CustomError(format!("Failed to convert Markdown: {}", e).into()))?;

            let maybe_cache_max_age = &target.cache_control_max_age_in_seconds;

            let mut builder = Response::builder()
                .status(200)
                .header("Content-Type", "text/html; charset=utf-8")
                .header("ETag", &etag)
                .header("Last-Modified", &last_modified)
                .header("cache-key", HeaderValue::from_str(&cache_key).unwrap())
                .header("Vary", 
                        HeaderValue::from(COOKIE)
                        );

            if let Some(max_age) = maybe_cache_max_age {
                let cache_control_header = format!("public, max-age={}, immutable", max_age);
                builder = builder.header("Cache-Control", cache_control_header);
            }

            let resp = builder
                .body(html.into_bytes().into())
                .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;

            RESPONSE_CACHE.insert(cache_key.clone(), ("text/html; charset=utf-8".into(), Instant::now(), resp.clone()));
            return create_simple_response_from_bytes(resp);
        }

        // Binary / static file branch
        let mut mime_type = from_path(&full_path).first_or_octet_stream();
        if full_path.extension().and_then(|e| e.to_str()).map(|v| v.eq_ignore_ascii_case("md")).unwrap_or(false) {
            mime_type = mime::TEXT_PLAIN_UTF_8;
        }
        let body_bytes: Arc<[u8]> = Arc::from(file_content);

        let resp = Response::builder()
            .status(200)
            .header("Content-Type", mime_type.to_string())
            .header("Cache-Control", "public, max-age=10, immutable")
            .header("ETag", &etag)
            .header("Last-Modified", &last_modified)
            .header("Vary", HeaderValue::from(COOKIE))
            .body(body_bytes.to_vec().into())
            .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;

        RESPONSE_CACHE.insert(cache_key.clone(), (mime_type.to_string(), Instant::now(), resp.clone()));
        return create_simple_response_from_bytes(resp);
    }
 
    else if full_path.is_dir() {

        if req_path.ends_with("/") == false {
            let response = hyper::Response::builder()
                .status(301)
                .header("Location", format!("{}/", req_path))
                .body("".into())
                .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;

            return create_simple_response_from_bytes(response);
        }

        // Check for default files before listing the directory
        let default_files = ["index.html", "index.htm", "index.md"];

        for default_file in &default_files {
            let default_file_path = full_path.join(default_file);

            if default_file_path.is_file() {

                // Serve the default file
                let file_content = fs::read(&default_file_path)
                    .map_err(|e| CustomError(format!("Failed to read default file: {}", e).into()))?;

                if default_file_path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                    // Convert Markdown to HTML
                    let markdown = String::from_utf8_lossy(&file_content);
                    let html = super::markdown_to_html(req_is_dark(&req),&target.host_name,&markdown)
                        .map_err(|e| CustomError(format!("Failed to convert Markdown to HTML: {}", e).into()))?;

                    let response = hyper::Response::builder()
                        .status(200)
                        .header("Content-Type", "text/html; charset=utf-8")
                        .body(html.into_bytes().into())
                        .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;

                    // Cache the response
                    {
                        RESPONSE_CACHE.insert(
                            cache_key.clone(),
                            (
                                "text/html; charset=utf-8".to_string(),
                                Instant::now(),
                                response.clone()
                            ),
                        );
                    }

                    return create_simple_response_from_bytes(response);
                }

                // Guess the MIME type
                let mime_type = from_path(&default_file_path).first_or_octet_stream();

                let response_bytes : Arc<[u8]> = Arc::from(file_content);


                let response = hyper::Response::builder()
                    .status(200)
                    .header("Content-Type", mime_type.to_string())
                    .body(response_bytes.to_vec().into())
                    .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;

                // Cache the response
                {
                    RESPONSE_CACHE.insert(
                        cache_key.clone(),
                        (
                            mime_type.to_string(),
                            Instant::now(),
                            response.clone()
                        ),
                    );
                }

                return create_simple_response_from_bytes(response);
            }
        }

        // prevent listing directories if not allowed by dir server configuration
        if target.enable_directory_browsing.unwrap_or_default() == false{
            let response_body = "Directory browsing is disabled";
            let response = hyper::Response::builder()
                .status(403)
                .header("Content-Type", "text/plain")
                .body(response_body.into())
                .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;
    
            return create_simple_response_from_bytes(response)
        }    

        
        create_index_page(&target,&full_path, &req_path,cache_key)

    } else {
        let resp = Response::builder()
            .status(404)
            .header("Content-Type", "text/plain")
            .body("Not Found".into())
            .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;
        create_simple_response_from_bytes(resp)
    }
}


// Function to format file sizes in a human-readable format
fn format_file_size(size: u64) -> String {
    let sizes = ["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut i = 0;
    while size >= 1024.0 && i < sizes.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    format!("{:.2} {}", size, sizes[i])
}


use std::collections::HashMap;

fn create_index_page(target:&DirServer,full_path: &PathBuf, req_path: &str, cache_key: String) -> Result<EpicResponse, CustomError> {
    
    // Read and collect all entries up front
    let mut content_in_the_directory: Vec<_> = fs::read_dir(&full_path)
        .map_err(|e| CustomError(format!("Failed to read directory: {}", e).into()))?
        .collect::<Result<_, _>>()
        .map_err(|e| CustomError(format!("Failed to read entry: {}", e).into()))?;

    // Sort so that directories come first, then files; each group alphabetically
    content_in_the_directory.sort_by(|a, b| {
        let a_is_dir = a.metadata().map(|m| m.is_dir()).unwrap_or(false);
        let b_is_dir = b.metadata().map(|m| m.is_dir()).unwrap_or(false);
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_name = a.file_name().to_string_lossy().to_lowercase();
                let b_name = b.file_name().to_string_lossy().to_lowercase();
                a_name.cmp(&b_name)
            }
        }
    });

    // Helper function to map file extensions to icons
    fn get_icon_for_extension(extension: &str) -> &'static str {
        let icons: HashMap<&str, &str> = [
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
        .iter()
        .cloned()
        .collect();

        icons.get(extension).cloned().unwrap_or("📄") // Default icon if not found
    }

    // Build the HTML response
    let html = Cow::Borrowed(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <!-- immediately paint according to system theme -->
    <script>
        const dark = localStorage.getItem("theme") == "dark" ?? window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
        document.documentElement.style.backgroundColor = dark ? '#161a29' : '#ffffff';
        document.documentElement.style.color            = dark ? '#ffffff' : '#000000';
    </script>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Directory Listing</title>
<style>
    body {
        padding: 2em;
        background-color: #161a29;
        color: #ffffff;
        font-family: Arial, sans-serif;
        margin: 0;
    }
    a {
        color: #1e90ff;
        text-decoration: none;
    }
    a:hover {
        text-decoration: underline;
    }
    ul {
        list-style-type: none;
        max-width: 1400px;
        margin: 0 auto; /* Centers the element horizontally */
        border-radius: 8px;
        padding: 4px; /* Adds padding inside the element for inner spacing */
        background-color: #161a29;
        background-image: 
            linear-gradient(45deg, #ff0000, #ff7f00, #ffff00, #00ff00, #0000ff, #4b0082, #8f00ff, #ff0000);
        background-size: 200% 200%;
        animation: rainbowBorder 10s alternate-reverse infinite;
        border: 4px solid transparent; /* Thick "border" effect */
        background-clip: padding-box, border-box; /* Keeps inner padding color separate */
    }
    li {
        display: flex;
        align-items: center;
        padding: 10px;
        border-bottom: 1px solid #333;
        background-color: #1e2336; /* Matches background for inner padding effect */
    }
    li:nth-child(even) {
        background-color: #0e1623;
    }
    .icon {
        margin-right: 10px;
        width: 16px;
        height: 16px;
    }
    .file-size {
        margin-left: auto;
        color: #888;
    }
    h1 {
        text-align: center;
        font-size: 1.5em;
        margin: 20px;
    }
    @media (max-width: 600px) {
        body {
            font-size: 0.9em;
        }
        .file-size {
            font-size: 0.8em;
        }
    }
    /* Keyframes for smooth gradient animation */
    @keyframes rainbowBorder {
        0% { background-position: 0% 50%; }
        100% { background-position: 100% 50%; }
    }
</style>
</head>
<body>
    <h1>Directory Listing for "#,
    );

    let mut html_owned = html.into_owned();
    html_owned.push_str(&req_path);
    html_owned.push_str(r#"</h1>
    <ul>"#);

    // Parent link if not root
    if req_path != "/" {
        let parent = Path::new(&req_path).parent().unwrap_or(Path::new("/"));
        let parent_str = parent.to_str().unwrap_or("/");
        html_owned.push_str(&format!(
            r#"<li><a href="{0}">.. (Parent Directory)</a></li>"#,
            parent_str
        ));
    }

    // Now iterate the **sorted** entries
    for entry in content_in_the_directory {
        let metadata = entry.metadata().map_err(|e| CustomError(format!("Failed to get metadata: {}", e).into()))?;
        let name = entry.file_name().into_string().unwrap_or_else(|_| "Unknown".into());
        let size_text = if metadata.is_file() {
            format!(" ({})", format_file_size(metadata.len()))
        } else {
            " (Directory)".to_string()
        };
        let path = format!("{}/{}", req_path.trim_end_matches('/'), name);
        let icon = if metadata.is_file() {
            let ext = name.rsplit('.').next().unwrap_or("");
            get_icon_for_extension(ext)
        } else {
            "📁"
        };

        html_owned.push_str(&format!(
            r#"<li><span class="icon">{2}</span><a href="{0}">{1}</a><span class="file-size">{3}</span></li>"#,
            path, name, icon, size_text
        ));
    }

    html_owned.push_str(
        r#"
    </ul>
</body>
</html>"#,
    );

    let maybe_cache_max_age = &target.cache_control_max_age_in_seconds;
    let mut builder = hyper::Response::builder()
        .status(200)
        .header("Content-Type", "text/html; charset=utf-8");

    if let Some(max_age) = maybe_cache_max_age {
        let cache_control_header = format!("public, max-age={}, immutable", max_age);
        builder = builder.header("Cache-Control", cache_control_header);
    }

    let response = builder
        .body(html_owned.into_bytes().into())
        .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;

    // Cache it
    RESPONSE_CACHE.insert(
        cache_key.clone(),
        ("text/html; charset=utf-8".into(), Instant::now(), response.clone()),
    );

    create_simple_response_from_bytes(response)
}