use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use hyper::Response;
use mime_guess::mime;
use crate::{
    configuration::v2::DirServer,
    http_proxy::{create_simple_response_from_bytes, EpicResponse},
    CustomError,
};


use once_cell::sync::Lazy;
type CacheValue = (String, Instant, Response<bytes::Bytes>); // (Content-Type, Cache Time, Response)
use dashmap::DashMap;
static RESPONSE_CACHE: Lazy<DashMap<String, CacheValue>> = Lazy::new(|| {
    DashMap::new()
});

// todo:
// - this endpoint is not counted towards any statistics or rate limits ... need to fix.
// - cors headers and such.
// - actual cache support using etag and such
// - markdown rendering (configurable)
// - directory listing opt in via config
// - compression
// - make sure we keep perf ok'ish. current impl sits around ~200k requests per second on an 8 core machine
//   serving some basic example site from "https://github.com/cloudacademy/static-website-example"
pub async fn handle(
    target: DirServer,
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<EpicResponse, CustomError> {
    use mime_guess::from_path;

    let root_dir = Path::new(&target.dir);

    let req_path = req.uri().path().to_string();

    let cache_key = req_path.clone();
    {
        tracing::trace!("checking cache for {}", cache_key);
        let mut expired_in_cache = false;
        if let Some(guard) = RESPONSE_CACHE.get(&cache_key) {
            
            let (_content_type, cache_time,res) = guard.value();
            // todo - configurable cache time
            if cache_time.elapsed() < Duration::from_secs(10) {
                tracing::trace!("cache hit for {}", cache_key);
                return create_simple_response_from_bytes(res.clone());
            } else {
                tracing::trace!("cache expired for {}", cache_key);
                expired_in_cache = true;
            }
        } else {
            tracing::trace!("cache miss for {}", cache_key);
        }
        if expired_in_cache {
            RESPONSE_CACHE.remove(&cache_key);
        }
    }

    tracing::trace!("fetching cold file");

    let requested_path = Path::new(&req_path);
    let full_path = root_dir.join(requested_path.strip_prefix("/").unwrap_or(requested_path));

    let full_path = match fs::canonicalize(&full_path) {
        Ok(path) => path,
        Err(e) => {
            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    let response_body = "sorry, there is nothing here..";
                    let response = Response::builder()
                        .status(404)
                        .header("Content-Type", "text/plain")
                        .body(response_body.into())
                        .expect("should always be possible to create 404 reply");

                    return create_simple_response_from_bytes(response);
                }
                _ => {}
            }
            return Err(CustomError(format!("Failed to canonicalize path: {}", e).into()));
        }
    };

    if !full_path.starts_with(root_dir) {
        return Err(CustomError("Attempted directory traversal".into()));
    }

    if full_path.is_file() {
        let file_content = fs::read(&full_path)
            .map_err(|e| CustomError(format!("Failed to read file: {}", e).into()))?;

        if target.render_markdown.unwrap_or_default() && full_path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            // Convert Markdown to HTML
            let markdown = String::from_utf8_lossy(&file_content);
            let html = super::markdown_to_html(full_path.file_name().unwrap_or_default().to_str().unwrap_or("unnamed"),&markdown)
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


        let mut mime_type = from_path(&full_path).first_or_octet_stream();

        if let Some(extension) = full_path.extension().and_then(|ext| ext.to_str()) {
            if extension.eq_ignore_ascii_case("md") {
                mime_type = mime::TEXT_PLAIN_UTF_8;
            }
        }

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
                   // response_bytes.clone(),
                    mime_type.to_string(),
                    Instant::now(),
                    response.clone()
                ),
            );
        }


        create_simple_response_from_bytes(response)
    } else if full_path.is_dir() {
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
                    let html = super::markdown_to_html("Index.md",&markdown)
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

        
        create_index_page(&full_path, &req_path,cache_key)

    } else {
        // Return 404 Not Found if the path is neither a file nor a directory
        let response_body = "Not Found";
        let response = hyper::Response::builder()
            .status(404)
            .header("Content-Type", "text/plain")
            .body(response_body.into())
            .map_err(|e| CustomError(format!("Failed to create response: {}", e).into()))?;

        create_simple_response_from_bytes(response)
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

fn create_index_page(full_path: &PathBuf, req_path: &str, cache_key: String) -> Result<EpicResponse, CustomError> {

    let content_in_the_directory = fs::read_dir(&full_path)
        .map_err(|e| CustomError(format!("Failed to read directory: {}", e).into()))?;

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
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Directory Listing</title>
<style>
    body {
        padding: 2em;
        background-color: #121212;
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
        background-color: #121212;
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
        background-color: #121212; /* Matches background for inner padding effect */
    }
    li:nth-child(even) {
        background-color: #1d1d1d;
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

    // Add a link to the parent directory if not at the root
    if req_path != "/" {
        let parent_path = Path::new(&req_path).parent().unwrap_or(Path::new("/"));
        let parent_path_str = parent_path.to_str().unwrap_or("/");
        html_owned.push_str(&format!(
            r#"<li><a href="{0}">.. (Parent Directory)</a></li>"#,
            parent_path_str
        ));
    }

    for entry in content_in_the_directory {
        let entry = entry.map_err(|e| CustomError(format!("Failed to read entry: {}", e).into()))?;
        let metadata = entry.metadata().map_err(|e| CustomError(format!("Failed to get metadata: {}", e).into()))?;
        let file_name = entry.file_name().into_string().unwrap_or_else(|_| "Unknown".into());
        let file_size = if metadata.is_file() {
            format!(" ({})", format_file_size(metadata.len()))
        } else {
            " (Directory)".to_string()
        };

        let entry_path = format!("{}/{}", req_path.trim_end_matches('/'), file_name);
        let icon = if metadata.is_file() {
            let extension = file_name.rsplit('.').next().unwrap_or("");
            get_icon_for_extension(extension)
        } else {
            "📁" // Folder icon
        };

        html_owned.push_str(&format!(
            r#"<li><span class="icon">{2}</span><a href="{0}">{1}</a><span class="file-size">{3}</span></li>"#,
            entry_path,
            file_name,
            icon,
            file_size
        ));
    }

    html_owned.push_str(
        r#"
        </ul>
    </body>
    </html>"#,
    );

    let response = hyper::Response::builder()
        .status(200)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(html_owned.into_bytes().into())
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
    create_simple_response_from_bytes(response)
}
