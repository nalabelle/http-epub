use anyhow::{Context, Result, anyhow};
use reqwest::blocking::Client;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct DownloadedImage {
    pub local_path: String,
    pub data: Vec<u8>,
    pub mime_type: &'static str,
}

#[derive(Clone, Debug)]
pub struct FetchedContent {
    pub original_url: Url,
    pub url: Url,
    pub html_string: String,
}

pub struct Fetcher {
    client: Client,
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn fetch_content(&self, url: &Url) -> Result<FetchedContent> {
        let pf_url = self.get_print_friendly_url(url);

        // Fetch the website content
        info!(url = %pf_url, "Fetching main HTML content...");
        let response = self
            .client
            .get(pf_url.clone())
            .send()
            .context("Failed to fetch website content")?;

        let html = response
            .text()
            .context("Failed to extract text from response")?;

        debug!(html_len = html.len(), "Main HTML content fetched.");

        Ok(FetchedContent {
            original_url: url.clone(),
            url: pf_url,
            html_string: html,
        })
    }

    /// Converts a regular URL to a print-friendly version if available
    pub fn get_print_friendly_url(&self, url: &Url) -> Url {
        let host = url.host_str().unwrap_or("");
        let _path = url.path();

        // Handle specific websites with known print-friendly versions
        if host.contains("wikipedia.org") {
            // Wikipedia: The printable version is deprecated, so we use the mobile version
            // which is cleaner and more suitable for reading
            let mut print_url = url.clone();
            if host.starts_with("en.") {
                // Convert en.wikipedia.org to en.m.wikipedia.org
                print_url.set_host(Some("en.m.wikipedia.org")).ok();
            } else if !host.contains(".m.") && host.contains(".") {
                // For other language wikis, insert 'm.' after the language code
                let parts: Vec<&str> = host.splitn(2, '.').collect();
                if parts.len() == 2 {
                    let lang = parts[0];
                    let domain = parts[1];
                    let mobile_host = format!("{}.m.{}", lang, domain);
                    print_url.set_host(Some(&mobile_host)).ok();
                }
            }
            return print_url;
        } else if host.contains("medium.com") {
            // Medium: Append ?format=print
            let mut print_url = url.clone();
            print_url.set_query(Some("format=print"));
            return print_url;
        } else if host.contains("nytimes.com") || host.contains("washingtonpost.com") {
            // News sites often have a print parameter
            let mut print_url = url.clone();
            if let Some(query) = url.query() {
                print_url.set_query(Some(&format!("{}&print=true", query)));
            } else {
                print_url.set_query(Some("print=true"));
            }
            return print_url;
        }

        // For other websites, return the original URL
        url.clone()
    }

    // Renamed from extract_and_download_images
    // Now takes a HashSet of specific Url objects to download and returns a map with DownloadedImage structs.
    pub fn download_image_list(
        &self,
        image_urls: &HashSet<Url>,
    ) -> Result<HashMap<String, DownloadedImage>> {
        let mut image_map = HashMap::new();
        info!(
            count = image_urls.len(),
            "Starting to download identified images..."
        );

        for url in image_urls {
            debug!(url = %url, "Attempting to download image.");
            match self.download_image(url) {
                Ok((image_binary_data, image_mime_type)) => {
                    let base_name = self.generate_unique_filename(url);
                    let extension = self.mime_type_to_extension(image_mime_type);
                    let local_img_path = format!("images/{}.{}", base_name, extension);

                    let downloaded_image_info = DownloadedImage {
                        local_path: local_img_path.clone(),
                        data: image_binary_data,
                        mime_type: image_mime_type,
                    };

                    debug!(
                        original_url = %url,
                        local_path = local_img_path,
                        "Image downloaded successfully."
                    );
                    image_map.insert(url.as_str().to_string(), downloaded_image_info);
                }
                Err(e) => {
                    warn!(url = %url, error = %e, "Failed to download image");
                }
            }
        }
        info!(
            downloaded_count = image_map.len(),
            "Finished downloading images."
        );
        Ok(image_map)
    }

    pub fn generate_unique_filename(&self, url: &Url) -> String {
        // Extract the filename from the URL or generate a unique ID
        url.path_segments()
            .and_then(|mut segments| segments.next_back())
            .and_then(|name| {
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            })
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    }

    pub fn mime_type_to_extension(&self, mime_type: &str) -> &str {
        match mime_type {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/svg+xml" => "svg",
            "image/webp" => "webp",
            _ => "jpg", // Default
        }
    }

    pub fn download_image(&self, img_url: &Url) -> Result<(Vec<u8>, &'static str)> {
        // Fetch the image
        let response = self
            .client
            .get(img_url.clone())
            .send()
            .context(format!("Failed to fetch image from {}", img_url))?;

        // Check if the request was successful
        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download image: HTTP status {}",
                response.status()
            ));
        }

        // Get content type
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg"); // Default to JPEG if no content type

        // Determine MIME type
        let mime_type = match content_type {
            t if t.contains("jpeg") || t.contains("jpg") => "image/jpeg",
            t if t.contains("png") => "image/png",
            t if t.contains("gif") => "image/gif",
            t if t.contains("svg") => "image/svg+xml",
            t if t.contains("webp") => "image/webp",
            _ => "image/jpeg", // Default
        };

        // Read the image data
        let data = response
            .bytes()
            .context(format!("Failed to read image data from {}", img_url))?;

        Ok((data.to_vec(), mime_type))
    }
}
