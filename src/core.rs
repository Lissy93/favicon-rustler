// Core business logic shared between Cloudflare Workers and standalone server
use image::{ImageError, ImageOutputFormat};
use serde_json::Value as JsonValue;
use soup::{NodeExt, QueryBuilderExt, Soup};
use url::Url as StdUrl;

#[derive(Debug)]
pub enum AppError {
    InvalidUrl(String),
    NetworkError(String),
    NotFound(String),
    ImageError(String),
    ParseError(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            AppError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::ImageError(msg) => write!(f, "Image error: {}", msg),
            AppError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<url::ParseError> for AppError {
    fn from(err: url::ParseError) -> Self {
        AppError::InvalidUrl(err.to_string())
    }
}

impl From<ImageError> for AppError {
    fn from(err: ImageError) -> Self {
        AppError::ImageError(err.to_string())
    }
}

/// Find icon URL from HTML content
pub fn find_icon_from_html(html: &str, base_url: &str) -> Result<Option<String>, AppError> {
    let soup = Soup::new(html);
    let base_url = StdUrl::parse(base_url)?;

    let icons = [
        ("apple-touch-icon", "href"),
        ("icon", "href"),
        ("shortcut icon", "href"),
    ];

    for &(icon, attr) in &icons {
        let elements = soup.tag(icon.split_once('[').unwrap_or((icon, "")).0).find_all();

        for element in elements {
            if let Some(link) = element.get(attr) {
                let full_url = if link.starts_with("http://") || link.starts_with("https://") {
                    link.to_string()
                } else {
                    base_url.join(&link)?.to_string()
                };
                return Ok(Some(full_url));
            }
        }
    }

    Ok(None)
}

/// Find icon URLs from manifest
pub fn find_icon_from_manifest(manifest_json: &str, base_url: &str) -> Result<Option<String>, AppError> {
    let base_url = StdUrl::parse(base_url).map_err(|e| AppError::ParseError(e.to_string()))?;
    let manifest: JsonValue = serde_json::from_str(manifest_json)
        .map_err(|e| AppError::ParseError(e.to_string()))?;

    if let Some(icons) = manifest["icons"].as_array() {
        for icon in icons {
            if let Some(icon_src) = icon["src"].as_str() {
                let full_url = if icon_src.starts_with("http://") || icon_src.starts_with("https://") {
                    icon_src.to_string()
                } else {
                    base_url
                        .join(icon_src)
                        .map_err(|e| AppError::ParseError(e.to_string()))?
                        .to_string()
                };
                return Ok(Some(full_url));
            }
        }
    }
    Ok(None)
}

/// Resize image to specified dimensions
pub fn resize_image(image_data: &[u8], size: u32) -> Result<Vec<u8>, AppError> {
    let img = image::load_from_memory(image_data)?;
    let scaled = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
    let mut result = Vec::new();
    scaled.write_to(&mut std::io::Cursor::new(&mut result), ImageOutputFormat::Png)?;
    Ok(result)
}

/// Well-known icon locations to check
pub fn get_well_known_icons() -> Vec<&'static str> {
    vec!["favicon.ico", "favicon.png", "apple-touch-icon.png"]
}

/// Validate and construct full URL
pub fn validate_and_construct_url(link: &str, base_url: &str) -> Result<String, AppError> {
    if link.starts_with("http://") || link.starts_with("https://") {
        Ok(link.to_string())
    } else {
        let base = StdUrl::parse(base_url)?;
        Ok(base.join(link)?.to_string())
    }
}
