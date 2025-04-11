use anyhow::{Context, Result, anyhow};
use regex::Regex;
use reqwest::blocking::Client;
use select::document::Document;
use std::collections::HashMap;
use url::Url;
use uuid::Uuid;

pub struct FetchedContent {
    pub document: Document,
    pub url: Url,
    pub images: HashMap<String, (String, Vec<u8>, &'static str)>
}

pub struct Fetcher {
    client: Client
}

impl Fetcher {
    pub fn new() -> Self {
        Self {
            client: Client::new()
        }
    }
    
    pub fn fetch_content(&self, url_str: &str) -> Result<FetchedContent> {
        // Parse the URL
        let mut url = Url::parse(url_str)
            .context("Failed to parse URL")?;
        
        // Always use print-friendly URL
        url = self.get_print_friendly_url(&url);
        println!("Using print-friendly URL: {}", url);
        
        // Fetch the website content
        println!("Fetching content from {}...", url);
        let response = self.client.get(url.clone())
            .send()
            .context("Failed to fetch website content")?;
        
        let html = response.text()
            .context("Failed to extract text from response")?;
        
        // Parse HTML
        let document = Document::from(html.as_str());
        
        // Extract and download images
        println!("Fetching images...");
        let images = self.extract_and_download_images(&html, &url)?;
        
        Ok(FetchedContent { document, url, images })
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

    pub fn extract_and_download_images(
        &self,
        html_content: &str, 
        base_url: &Url
    ) -> Result<HashMap<String, (String, Vec<u8>, &'static str)>> {
        let mut image_map = HashMap::new();
        
        // Find all image tags
        let img_regex = Regex::new(r#"<img[^>]*src=["']([^"']+)["'][^>]*>"#).unwrap();
        
        for cap in img_regex.captures_iter(html_content) {
            let img_src = cap.get(1).unwrap().as_str();
            
            // Skip data URLs
            if img_src.starts_with("data:") {
                continue;
            }
            
            // Convert relative URLs to absolute
            let img_url = match base_url.join(img_src) {
                Ok(url) => url,
                Err(e) => {
                    eprintln!("Warning: Failed to parse image URL {}: {}", img_src, e);
                    continue;
                }
            };
            
            // Download image
            match self.download_image(&img_url) {
                Ok((data, mime_type)) => {
                    // Generate a unique filename
                    let base_name = self.generate_unique_filename(&img_url);
                    let extension = self.mime_type_to_extension(mime_type);
                    let img_path = format!("images/{}.{}", base_name, extension);
                    
                    // Add to map
                    image_map.insert(img_src.to_string(), (img_path, data, mime_type));
                },
                Err(e) => {
                    eprintln!("Warning: Failed to download image {}: {}", img_url, e);
                }
            }
        }
        
        Ok(image_map)
    }

    pub fn generate_unique_filename(&self, url: &Url) -> String {
        // Extract the filename from the URL or generate a unique ID
        url.path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| {
                if name.is_empty() { None } else { Some(name.to_string()) }
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
            _ => "jpg",  // Default
        }
    }

    pub fn download_image(&self, img_url: &Url) -> Result<(Vec<u8>, &'static str)> {
        // Fetch the image
        let response = self.client.get(img_url.clone())
            .send()
            .context(format!("Failed to fetch image from {}", img_url))?;
        
        // Check if the request was successful
        if !response.status().is_success() {
            return Err(anyhow!("Failed to download image: HTTP status {}", response.status()));
        }
        
        // Get content type
        let content_type = response.headers()
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
        let data = response.bytes()
            .context(format!("Failed to read image data from {}", img_url))?;
        
        Ok((data.to_vec(), mime_type))
    }
}
