use axum::{
    extract::{Path, Query},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use favicon_rustler::core::{
    find_icon_from_html, find_icon_from_manifest, get_well_known_icons, resize_image,
    validate_and_construct_url,
};
use serde::Deserialize;
use soup::{NodeExt, QueryBuilderExt};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

// Server-specific error type
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

impl From<image::ImageError> for AppError {
    fn from(err: image::ImageError) -> Self {
        AppError::ImageError(err.to_string())
    }
}

impl From<favicon_rustler::core::AppError> for AppError {
    fn from(err: favicon_rustler::core::AppError) -> Self {
        match err {
            favicon_rustler::core::AppError::InvalidUrl(msg) => AppError::InvalidUrl(msg),
            favicon_rustler::core::AppError::NetworkError(msg) => AppError::NetworkError(msg),
            favicon_rustler::core::AppError::NotFound(msg) => AppError::NotFound(msg),
            favicon_rustler::core::AppError::ImageError(msg) => AppError::ImageError(msg),
            favicon_rustler::core::AppError::ParseError(msg) => AppError::ParseError(msg),
        }
    }
}

#[derive(Debug, Deserialize)]
struct IconParams {
    size: Option<u32>,
    fallback: Option<String>,
    shape: Option<String>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Build CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build application routes
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/:website", get(get_icon_default))
        .route("/:website/:size", get(get_icon_with_size))
        .layer(cors);

    // Get port from environment or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting favicon-rustler server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        "Favicon Rustler API\n\nEndpoints:\n  GET /:website - Get default favicon\n  GET /:website/:size - Get favicon at specific size\n"
    )
}

#[axum::debug_handler]
async fn get_icon_default(
    Path(website): Path<String>,
    Query(params): Query<IconParams>,
) -> Response {
    let size = params.size.unwrap_or(64);
    match get_icon(&website, size, params.fallback).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

#[axum::debug_handler]
async fn get_icon_with_size(
    Path((website, size)): Path<(String, u32)>,
    Query(params): Query<IconParams>,
) -> Response {
    match get_icon(&website, size, params.fallback).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn get_icon(
    website: &str,
    size: u32,
    _fallback: Option<String>,
) -> Result<Response, AppError> {
    // Validate size
    if size > 512 {
        return Err(AppError::InvalidUrl("Maximum size is 512 pixels".to_string()));
    }

    // Build target URL
    let target_url = if website.starts_with("http://") || website.starts_with("https://") {
        website.to_string()
    } else {
        format!("https://{}", website)
    };

    // Check if website is accessible
    match check_website(&target_url).await {
        Ok(false) => return Err(AppError::NotFound("Website is not accessible".to_string())),
        Err(e) => return Err(AppError::NetworkError(format!("Failed to check website: {}", e))),
        _ => {}
    }

    // Find icon URL
    let icon_url = match find_icon_url(&target_url).await {
        Ok(Some(url)) => url,
        Ok(None) => return Err(AppError::NotFound("No icon found".to_string())),
        Err(e) => return Err(e),
    };

    // Fetch and resize icon
    let icon_data = fetch_and_resize_icon(&icon_url, size).await?;

    // Build response
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=86400".parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, icon_data).into_response())
}

async fn check_website(url: &str) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(response.status().is_success())
}

async fn find_icon_url(url: &str) -> Result<Option<String>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::NetworkError(e.to_string()))?;

    // Fetch HTML
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(e.to_string()))?;

    let html = response
        .text()
        .await
        .map_err(|e| AppError::NetworkError(e.to_string()))?;

    // Try to find icon in HTML
    if let Some(icon_url) = find_icon_from_html(&html, url)? {
        return Ok(Some(icon_url));
    }

    // Check for manifest
    let soup = soup::Soup::new(&html);
    if let Some(element) = soup.tag("link").attr("rel", "manifest").find() {
        if let Some(manifest_link) = element.get("href") {
            let manifest_url = validate_and_construct_url(&manifest_link, url)?;
        let manifest_response = client
            .get(&manifest_url)
            .send()
            .await
            .map_err(|e| AppError::NetworkError(e.to_string()))?;

        let manifest_text = manifest_response
            .text()
            .await
            .map_err(|e| AppError::NetworkError(e.to_string()))?;

            if let Some(icon_url) = find_icon_from_manifest(&manifest_text, url)? {
                return Ok(Some(icon_url));
            }
        }
    }

    // Check well-known locations
    for icon_path in get_well_known_icons() {
        let icon_url = validate_and_construct_url(icon_path, url)?;
        if check_url_exists(&client, &icon_url).await.unwrap_or(false) {
            return Ok(Some(icon_url));
        }
    }

    // Fallback to Google's favicon service
    let fallback_url = format!(
        "https://t3.gstatic.com/faviconV2?client=SOCIAL&type=FAVICON&fallback_opts=TYPE,SIZE,URL&url={}&size=128",
        url
    );

    Ok(Some(fallback_url))
}

async fn check_url_exists(client: &reqwest::Client, url: &str) -> Result<bool, String> {
    let response = client.head(url).send().await.map_err(|e| e.to_string())?;
    Ok(response.status().is_success())
}

async fn fetch_and_resize_icon(url: &str, size: u32) -> Result<Vec<u8>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::NetworkError(e.to_string()))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(AppError::NetworkError(format!(
            "Failed to fetch icon: HTTP {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::NetworkError(e.to_string()))?;

    Ok(resize_image(&bytes, size)?)
}

// Implement IntoResponse for AppError
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::InvalidUrl(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::NetworkError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::ImageError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, message).into_response()
    }
}
