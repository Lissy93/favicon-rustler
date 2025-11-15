use worker::*;
use std::fmt;
use soup::{Soup, QueryBuilderExt, NodeExt};
use url::Url as StdUrl;
use image::{ImageError, ImageOutputFormat};
use serde_json::Value as JsonValue;

pub async fn is_website_up(url: &str) -> Result<bool> {
    let response = Fetch::Url(Url::parse(url)?).send().await?;
    Ok(response.status_code() >= 200 && response.status_code() < 300)
}

pub struct MyImageError(ImageError);

impl From<MyImageError> for worker::Error {
    fn from(err: MyImageError) -> Self {
        worker::Error::from(err.0.to_string())
    }
}

impl fmt::Display for MyImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

/// Finds the icon URL from the given website's HTML content or manifest
pub async fn find_icon_url(url: &str) -> Result<Option<String>> {
  let mut response = Fetch::Url(Url::parse(url)?).send().await?;
  let html = response.text().await?;
  let soup = Soup::new(&html);
  let base_url = StdUrl::parse(url)?;

  let icons = [
      ("apple-touch-icon", "href"),
      ("icon", "href"),
      ("shortcut icon", "href"),
      ("link[rel='manifest']", "href"),
  ];

  console_log!("Fetching icon from URL: {}", url);

  for &(icon, attr) in &icons {
      console_log!("Searching for icons of type: {}", icon);
      let elements = soup.tag(icon.split_once('[').unwrap_or((icon, "")).0).find_all();

      for element in elements {
          if let Some(link) = element.get(attr) {
              if icon == "link[rel='manifest']" {
                  if let Ok(Some(icon_url)) = process_manifest(&link, &base_url).await {
                      return Ok(Some(icon_url));
                  }
              } else {
                  let full_url = match link.starts_with("http://") || link.starts_with("https://") {
                      true => link.to_string(),  // Already absolute URL
                      false => base_url.join(&link)?.to_string(),  // Resolve relative URL
                  };
                  console_log!("Icon found: {}", full_url);
                  return Ok(Some(full_url));
              }
          }
      }
  }

  // Check well-known locations
  let well_known_icons = ["favicon.ico", "favicon.png", "apple-touch-icon.png"];
  for icon in well_known_icons.iter() {
      let icon_url = base_url.join(icon)?;

      if let Ok(true) = check_url_exists(&icon_url.to_string()).await {
        console_log!("Icon found in well-known location: {}", icon_url);
        return Ok(Some(icon_url.to_string()));
      } else {
        console_log!("Failed to find icon in well-known location: {}", icon_url);
      }
  }

  // Specific handling for meta tags with property="og:image"
  if let Some(og_image) = soup.tag("meta").attr("property", "og:image").find() {
    if let Some(content) = og_image.get("content") {
        let full_url = validate_and_construct_url(&content, &base_url)?;
        console_log!("OG Image found: {}", full_url);
        return Ok(Some(full_url));
    }
  }

  // Fallback to external service
  let fallback_url = format!("https://t3.gstatic.com/faviconV2?client=SOCIAL&type=FAVICON&fallback_opts=TYPE,SIZE,URL&url={}&size=128", url);
  if let Ok(true) = check_url_exists(&fallback_url).await {
    console_log!("Icon found using fallback service: {}", fallback_url);
    return Ok(Some(fallback_url));
  } else {
    console_log!("Failed to verify icon at fallback service: {}", fallback_url);
  }
  console_log!("No icon found for URL: {}", url);
  Ok(None)
}

/// Validate and construct the full URL from a potential relative link
fn validate_and_construct_url(link: &str, base_url: &StdUrl) -> Result<String> {
  if link.starts_with("http://") || link.starts_with("https://") {
      Ok(link.to_string())
  } else {
      base_url.join(link).map_err(|e| worker::Error::from(e.to_string())).map(|url| url.to_string())
  }
}

/// Processes a manifest file to find icons
async fn process_manifest(manifest_url: &str, base_url: &StdUrl) -> Result<Option<String>> {
  // Parse the URL from the string, handling errors appropriately
  let parsed_url = StdUrl::parse(manifest_url).map_err(|e| worker::Error::from(e.to_string()))?;

  // Perform the fetch operation
  let mut response = Fetch::Url(Url::parse(&parsed_url.to_string())?)
      .send()
      .await
      .map_err(|e| worker::Error::from(e.to_string()))?;

  // Attempt to deserialize the JSON response
  let manifest: JsonValue = response.json::<JsonValue>().await
      .map_err(|e| worker::Error::from(e.to_string()))?;

  // Look for the 'icons' array in the JSON structure
  if let Some(icons) = manifest["icons"].as_array() {
      for icon in icons {
          if let Some(icon_src) = icon["src"].as_str() {
              // Construct the full URL based on whether it's relative or absolute
              let full_url = match icon_src.starts_with("http://") || icon_src.starts_with("https://") {
                  true => icon_src.to_string(),
                  false => base_url.join(icon_src).map_err(|e| worker::Error::from(e.to_string()))?.to_string(),
              };
              return Ok(Some(full_url));
          }
      }
  }
  Ok(None)
}


/// Checks if a URL exists by performing a HEAD request
async fn check_url_exists(url: &str) -> Result<bool> {
  let parsed_url = StdUrl::parse(url).map_err(|e| worker::Error::from(e.to_string()))?;
  let request = Request::new(&parsed_url.to_string(), Method::Head)?;
  let response = Fetch::Request(request).send().await.map_err(|e| worker::Error::from(e.to_string()))?;
  Ok(response.status_code() == 200)
}


/// Fetches an image from the given URL, resizes it, and returns the raw bytes of the resized image.
pub async fn fetch_and_scale_icon(url: &str, size: u32) -> Result<Vec<u8>> {
    let mut response = Fetch::Url(Url::parse(url)?).send().await?;
    if response.status_code() >= 200 && response.status_code() < 300 {
        let bytes = response.bytes().await?;
        resize_image(&bytes, size)  // Directly use the function without external map_err
    } else {
        Err(worker::Error::from(format!("Failed to fetch the original image: HTTP {}", response.status_code())))
    }
}


/// Resizes the image to the specified dimensions using the `image` crate.
fn resize_image(image_data: &[u8], size: u32) -> Result<Vec<u8>> {
    let img = image::load_from_memory(image_data).map_err(MyImageError)?;
    let scaled = img.resize_exact(size, size, image::imageops::FilterType::Nearest);
    let mut result = Vec::new();
    scaled.write_to(&mut result, ImageOutputFormat::Png).map_err(MyImageError)?;
    Ok(result)
}
