//! Body content inspection — detect content kind from HTTP headers and magic
//! bytes, decode images for inline rendering, and provide hex previews for
//! binary data.

use iced::widget::image as iced_image;

// ─── Content kind ────────────────────────────────────────────────────────────

/// High-level classification of a body's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyContentKind {
    Json,
    Xml,
    Html,
    Css,
    JavaScript,
    PlainText,
    FormUrlEncoded,
    Multipart,
    Image(ImageFormat),
    Svg,
    Pdf,
    Wasm,
    Protobuf,
    MsgPack,
    /// Recognised as binary but not a specific type we handle specially.
    Binary,
}

impl BodyContentKind {
    /// Human-readable label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Xml => "XML",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::JavaScript => "JavaScript",
            Self::PlainText => "Plain Text",
            Self::FormUrlEncoded => "Form URL-Encoded",
            Self::Multipart => "Multipart Form",
            Self::Image(fmt) => fmt.label(),
            Self::Svg => "SVG",
            Self::Pdf => "PDF",
            Self::Wasm => "WebAssembly",
            Self::Protobuf => "Protobuf",
            Self::MsgPack => "MessagePack",
            Self::Binary => "Binary",
        }
    }

    /// `true` when the body is textual and should be rendered in a code block.
    pub fn is_text(self) -> bool {
        matches!(
            self,
            Self::Json
                | Self::Xml
                | Self::Html
                | Self::Css
                | Self::JavaScript
                | Self::PlainText
                | Self::FormUrlEncoded
                | Self::Svg
        )
    }

    /// `true` when we can decode and render the body as an inline image.
    pub fn is_inline_image(self) -> bool {
        matches!(self, Self::Image(_))
    }
}

/// Image formats we can decode for inline rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Webp,
    Bmp,
    Ico,
}

impl ImageFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jpeg => "JPEG",
            Self::Png => "PNG",
            Self::Gif => "GIF",
            Self::Webp => "WebP",
            Self::Bmp => "BMP",
            Self::Ico => "ICO",
        }
    }

    /// Suggested file extension (without dot).
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Ico => "ico",
        }
    }
}

// ─── Detection ───────────────────────────────────────────────────────────────

/// Detect the content kind of a body using (in priority order):
///
/// 1. The `Content-Type` HTTP header value.
/// 2. Magic-byte signatures at the start of `bytes`.
/// 3. UTF-8 validity + structural heuristics (leading `{`, `<`, etc.).
pub fn detect_content_kind(content_type_header: Option<&str>, bytes: &[u8]) -> BodyContentKind {
    // ── 1. Content-Type header ───────────────────────────────────
    if let Some(ct) = content_type_header {
        if let Some(kind) = detect_from_content_type(ct) {
            return kind;
        }
    }

    // ── 2. Magic bytes ───────────────────────────────────────────
    if let Some(kind) = detect_from_magic(bytes) {
        return kind;
    }

    // ── 3. UTF-8 heuristics ──────────────────────────────────────
    detect_from_content(bytes)
}

/// Try to map a `Content-Type` header value to a [`BodyContentKind`].
fn detect_from_content_type(ct: &str) -> Option<BodyContentKind> {
    // Normalise: lowercase, strip parameters (charset, boundary, …)
    let ct = ct.to_ascii_lowercase();
    let mime = ct.split(';').next().unwrap_or(&ct).trim();

    // Text types
    if mime == "application/json" || mime == "text/json" || mime.ends_with("+json") {
        return Some(BodyContentKind::Json);
    }
    // SVG must be checked before generic +xml so image/svg+xml isn't
    // misclassified as plain XML.
    if mime == "image/svg+xml" {
        return Some(BodyContentKind::Svg);
    }
    if mime == "text/xml" || mime == "application/xml" || mime.ends_with("+xml") {
        return Some(BodyContentKind::Xml);
    }
    if mime == "text/html" || mime == "application/xhtml+xml" {
        return Some(BodyContentKind::Html);
    }
    if mime == "text/css" {
        return Some(BodyContentKind::Css);
    }
    if mime == "application/javascript"
        || mime == "text/javascript"
        || mime == "application/x-javascript"
    {
        return Some(BodyContentKind::JavaScript);
    }
    if mime == "text/plain" {
        return Some(BodyContentKind::PlainText);
    }
    if mime == "application/x-www-form-urlencoded" {
        return Some(BodyContentKind::FormUrlEncoded);
    }
    if mime.starts_with("multipart/") {
        return Some(BodyContentKind::Multipart);
    }
    // Image types
    if mime == "image/jpeg" || mime == "image/jpg" {
        return Some(BodyContentKind::Image(ImageFormat::Jpeg));
    }
    if mime == "image/png" {
        return Some(BodyContentKind::Image(ImageFormat::Png));
    }
    if mime == "image/gif" {
        return Some(BodyContentKind::Image(ImageFormat::Gif));
    }
    if mime == "image/webp" {
        return Some(BodyContentKind::Image(ImageFormat::Webp));
    }
    if mime == "image/bmp" || mime == "image/x-bmp" {
        return Some(BodyContentKind::Image(ImageFormat::Bmp));
    }
    if mime == "image/x-icon" || mime == "image/vnd.microsoft.icon" {
        return Some(BodyContentKind::Image(ImageFormat::Ico));
    }

    // Other binary types we recognise
    if mime == "application/pdf" {
        return Some(BodyContentKind::Pdf);
    }
    if mime == "application/wasm" {
        return Some(BodyContentKind::Wasm);
    }
    if mime == "application/x-protobuf"
        || mime == "application/protobuf"
        || mime == "application/grpc"
        || mime == "application/grpc+proto"
    {
        return Some(BodyContentKind::Protobuf);
    }
    if mime == "application/x-msgpack" || mime == "application/msgpack" {
        return Some(BodyContentKind::MsgPack);
    }

    // Generic text/* → plain text
    if mime.starts_with("text/") {
        return Some(BodyContentKind::PlainText);
    }

    // application/octet-stream or anything we don't recognise: fall through
    // to magic-byte / content detection.
    None
}

/// Try to identify the content from the first few bytes (magic signatures).
fn detect_from_magic(bytes: &[u8]) -> Option<BodyContentKind> {
    if bytes.len() < 4 {
        return None;
    }

    // JPEG: FF D8 FF
    if bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some(BodyContentKind::Image(ImageFormat::Jpeg));
    }
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some(BodyContentKind::Image(ImageFormat::Png));
    }
    // GIF: GIF87a or GIF89a
    if bytes.starts_with(b"GIF8")
        && bytes.len() >= 6
        && (bytes[4] == b'7' || bytes[4] == b'9')
    {
        return Some(BodyContentKind::Image(ImageFormat::Gif));
    }
    // WebP: RIFF....WEBP
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        return Some(BodyContentKind::Image(ImageFormat::Webp));
    }
    // BMP: BM
    if bytes[0] == b'B' && bytes[1] == b'M' && bytes.len() >= 14 {
        return Some(BodyContentKind::Image(ImageFormat::Bmp));
    }
    // ICO: 00 00 01 00
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some(BodyContentKind::Image(ImageFormat::Ico));
    }
    // PDF: %PDF
    if bytes.starts_with(b"%PDF") {
        return Some(BodyContentKind::Pdf);
    }
    // WASM: \0asm
    if bytes.starts_with(&[0x00, 0x61, 0x73, 0x6D]) {
        return Some(BodyContentKind::Wasm);
    }
    // Gzip: 1F 8B — we skip gzip here because the caller should have
    // already decompressed.

    None
}

/// Heuristic detection based on the body bytes themselves (UTF-8 validity,
/// leading characters, etc.).
fn detect_from_content(bytes: &[u8]) -> BodyContentKind {
    if bytes.is_empty() {
        return BodyContentKind::Binary;
    }

    // Check UTF-8 validity on a generous sample
    let sample = &bytes[..bytes.len().min(4096)];
    if std::str::from_utf8(sample).is_ok() {
        // Skip leading whitespace / BOM for structural checks
        let trimmed = sample
            .strip_prefix(&[0xEF, 0xBB, 0xBF]) // UTF-8 BOM
            .unwrap_or(sample);
        let trimmed = std::str::from_utf8(trimmed).unwrap_or("").trim_start();

        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return BodyContentKind::Json;
        }
        if trimmed.starts_with("<!DOCTYPE html")
            || trimmed.starts_with("<!doctype html")
            || trimmed.starts_with("<html")
            || trimmed.starts_with("<HTML")
        {
            return BodyContentKind::Html;
        }
        if trimmed.starts_with("<?xml") || trimmed.starts_with("<svg") {
            if trimmed.starts_with("<svg") {
                return BodyContentKind::Svg;
            }
            return BodyContentKind::Xml;
        }
        if trimmed.starts_with('<') {
            // Could be HTML fragment, XML, or other markup
            return BodyContentKind::Html;
        }

        return BodyContentKind::PlainText;
    }

    BodyContentKind::Binary
}

// ─── Image decoding ──────────────────────────────────────────────────────────

/// Maximum dimensions we'll allow for an inline image preview.
/// Images larger than this are scaled down to avoid blowing up memory.
const MAX_IMAGE_DIMENSION: u32 = 1024;

/// Try to decode `raw_bytes` as an image and produce an `iced_image::Handle`
/// ready for inline rendering.
///
/// Returns `None` if decoding fails (corrupt data, unsupported variant, etc.).
pub fn try_decode_image(raw_bytes: &[u8]) -> Option<DecodedImage> {
    let img = image::load_from_memory(raw_bytes).ok()?;
    let (w, h) = (img.width(), img.height());

    // Scale down if needed
    let img = if w > MAX_IMAGE_DIMENSION || h > MAX_IMAGE_DIMENSION {
        img.resize(
            MAX_IMAGE_DIMENSION,
            MAX_IMAGE_DIMENSION,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let handle = iced_image::Handle::from_rgba(width, height, rgba.into_raw());

    Some(DecodedImage {
        handle,
        original_width: w,
        original_height: h,
        display_width: width,
        display_height: height,
    })
}

/// A successfully decoded image ready for iced rendering.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DecodedImage {
    /// The iced image handle (RGBA pixel data).
    pub handle: iced_image::Handle,
    /// Original image dimensions before any resize.
    pub original_width: u32,
    pub original_height: u32,
    /// Dimensions of the image as stored in the handle (may be scaled down).
    pub display_width: u32,
    pub display_height: u32,
}

// ─── Hex preview ─────────────────────────────────────────────────────────────

/// Maximum number of bytes shown in a hex preview.
const HEX_PREVIEW_BYTES: usize = 256;

/// Format a binary body as a hex-dump string (address + hex + ASCII), similar
/// to `xxd` or `hexdump -C`.
///
/// Example output:
/// ```text
/// 00000000  89 50 4e 47 0d 0a 1a 0a  00 00 00 0d 49 48 44 52  |.PNG........IHDR|
/// 00000010  00 00 01 00 00 00 01 00  08 06 00 00 00 5c 72 a8  |.............r.|
/// ```
pub fn hex_preview(bytes: &[u8], max_bytes: Option<usize>) -> String {
    let limit = max_bytes.unwrap_or(HEX_PREVIEW_BYTES).min(bytes.len());
    let data = &bytes[..limit];
    let mut out = String::with_capacity(limit * 5);

    for (i, chunk) in data.chunks(16).enumerate() {
        let offset = i * 16;

        // Address column
        out.push_str(&format!("{offset:08x}  "));

        // Hex columns (two groups of 8)
        for (j, byte) in chunk.iter().enumerate() {
            out.push_str(&format!("{byte:02x} "));
            if j == 7 {
                out.push(' ');
            }
        }
        // Pad if the last chunk is short
        if chunk.len() < 16 {
            for j in chunk.len()..16 {
                out.push_str("   ");
                if j == 7 {
                    out.push(' ');
                }
            }
        }

        // ASCII column
        out.push(' ');
        out.push('|');
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                out.push(*byte as char);
            } else {
                out.push('.');
            }
        }
        out.push('|');
        out.push('\n');
    }

    if limit < bytes.len() {
        out.push_str(&format!("… {} more bytes not shown\n", bytes.len() - limit));
    }

    out
}

// ─── Text preview helpers ────────────────────────────────────────────────────

/// Produce a text preview string from raw (already-decompressed) bytes, capped
/// at `max_chars` characters.
pub fn text_preview(bytes: &[u8], max_chars: usize) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    match std::str::from_utf8(bytes) {
        Ok(s) => truncate_str(s, max_chars),
        Err(_) => {
            let lossy = String::from_utf8_lossy(bytes);
            truncate_str(&lossy, max_chars)
        }
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    // Find a char boundary at or before max_chars
    let mut end = max_chars;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut preview = s[..end].to_string();
    preview.push('…');
    preview
}

/// Suggest a file extension for a save dialog based on the content kind.
pub fn suggest_extension(kind: BodyContentKind) -> &'static str {
    match kind {
        BodyContentKind::Json => "json",
        BodyContentKind::Xml => "xml",
        BodyContentKind::Html => "html",
        BodyContentKind::Css => "css",
        BodyContentKind::JavaScript => "js",
        BodyContentKind::PlainText => "txt",
        BodyContentKind::FormUrlEncoded => "txt",
        BodyContentKind::Multipart => "bin",
        BodyContentKind::Svg => "svg",
        BodyContentKind::Image(fmt) => fmt.extension(),
        BodyContentKind::Pdf => "pdf",
        BodyContentKind::Wasm => "wasm",
        BodyContentKind::Protobuf => "bin",
        BodyContentKind::MsgPack => "msgpack",
        BodyContentKind::Binary => "bin",
    }
}

/// Suggest a filter label + extensions list for a save-file dialog.
pub fn suggest_save_filter(kind: BodyContentKind) -> (&'static str, &'static [&'static str]) {
    match kind {
        BodyContentKind::Json => ("JSON files", &["json"]),
        BodyContentKind::Xml => ("XML files", &["xml"]),
        BodyContentKind::Html => ("HTML files", &["html", "htm"]),
        BodyContentKind::Css => ("CSS files", &["css"]),
        BodyContentKind::JavaScript => ("JavaScript files", &["js"]),
        BodyContentKind::PlainText => ("Text files", &["txt"]),
        BodyContentKind::FormUrlEncoded => ("Text files", &["txt"]),
        BodyContentKind::Multipart => ("Binary files", &["bin"]),
        BodyContentKind::Svg => ("SVG files", &["svg"]),
        BodyContentKind::Image(ImageFormat::Jpeg) => ("JPEG images", &["jpg", "jpeg"]),
        BodyContentKind::Image(ImageFormat::Png) => ("PNG images", &["png"]),
        BodyContentKind::Image(ImageFormat::Gif) => ("GIF images", &["gif"]),
        BodyContentKind::Image(ImageFormat::Webp) => ("WebP images", &["webp"]),
        BodyContentKind::Image(ImageFormat::Bmp) => ("BMP images", &["bmp"]),
        BodyContentKind::Image(ImageFormat::Ico) => ("Icon files", &["ico"]),
        BodyContentKind::Pdf => ("PDF files", &["pdf"]),
        BodyContentKind::Wasm => ("WebAssembly files", &["wasm"]),
        BodyContentKind::Protobuf => ("Binary files", &["bin"]),
        BodyContentKind::MsgPack => ("MessagePack files", &["msgpack"]),
        BodyContentKind::Binary => ("All files", &["bin", "*"]),
    }
}

/// Format a byte size into a human-readable string.
pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}