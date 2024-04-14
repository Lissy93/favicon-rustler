use worker::*;
mod utils;
use utils::{find_icon_url, fetch_and_scale_icon};

#[event(fetch)]
pub async fn main(req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path_segments = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap_or_default();

    if path_segments.len() < 2 {
        return Response::error("URL must be in the format /[url-to-website]/[size]", 400);
    }

    let target_url = format!("https://{}", path_segments[0]);
    let size = path_segments.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(64);

    if size > 512 {
        return Response::error("Maximum size is 512 pixels", 400);
    }
    
    match utils::is_website_up(&target_url).await {
        Ok(true) => {},
        Ok(false) => return Response::error("Website is not accessible", 404),
        Err(_) => return Response::error("Failed to check website accessibility", 500),
    }

    let icon_url = match find_icon_url(&target_url).await {
        Ok(Some(url)) => url,
        Ok(None) => return Response::error("No icon found", 404),
        Err(_) => return Response::error("Error finding icon", 500),
    };

    match fetch_and_scale_icon(&icon_url, size).await {
        Ok(data) => {
            let mut headers = Headers::new();
            headers.set("Content-Type", "image/png")?;
            Response::from_bytes(data).map(|resp| resp.with_headers(headers))
        },
        Err(_) => Response::error("Failed to fetch or scale the icon", 500),
    }
}
