use anyhow::Result;
use regex::Regex;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use std::collections::HashMap;
use crate::fetch::FetchedContent;

// Embed the template file directly into the binary
const TEMPLATE_HTML: &str = include_str!("template.html");

pub struct ExtractedContent {
    pub content: String,
    pub image_map: HashMap<String, (String, Vec<u8>, &'static str)>,
    pub title: String
}

pub struct Extractor<'a> {
    fetched: &'a FetchedContent,
}

impl<'a> Extractor<'a> {
    pub fn new(fetched: &'a FetchedContent) -> Self {
        Self { fetched }
    }
    
    pub fn extract_title(&self) -> String {
        // Try to extract title from HTML
        let title = self.fetched.document.find(Name("title"))
            .next()
            .map(|node| node.text().trim().to_string());
        
        // Fallback to h1 if title tag is not found
        let h1_title = self.fetched.document.find(Name("h1"))
            .next()
            .map(|node| node.text().trim().to_string());
        
        // Use the first option that exists, or fallback to domain name
        title.or(h1_title).unwrap_or_else(|| {
            self.fetched.url.domain()
                .unwrap_or("website")
                .to_string()
        })
    }
    
    pub fn get_title(&self, user_title: Option<&String>) -> String {
        // Use user-provided title if available, otherwise extract from document
        match user_title {
            Some(title) => title.clone(),
            None => self.extract_title(),
        }
    }
    
    pub fn process(&self, user_title: Option<&String>) -> Result<ExtractedContent> {
        // Get the title
        let title = self.get_title(user_title);
        
        // Process the document
        let extracted = self.process_document(&title)?;
        
        Ok(extracted)
    }

    pub fn process_document(
        &self,
        title: &str
    ) -> Result<ExtractedContent> {
        // Extract main content
        let mut content = self.extract_content_from_document(&self.fetched.document, title)?;
        
        // Replace image URLs with local references
        content = replace_image_urls(&content, &self.fetched.images);
        
        Ok(ExtractedContent {
            content,
            image_map: self.fetched.images.clone(),
            title: title.to_string()
        })
    }
    

    
    pub fn extract_content_from_document(&self, document: &Document, title: &str) -> Result<String> {
        // Try to find main content container
        // This is a simple implementation - real-world usage might need more sophisticated content extraction
        let content_selectors = [
            // Common content containers
            "article", "main", "#content", ".content", ".post", ".entry",
            // Fallbacks
            "body"
        ];
        
        for selector in content_selectors {
            let content = if selector.starts_with("#") {
                // ID selector
                document.find(Name("div").and(Class(selector.trim_start_matches("#"))))
                    .next()
                    .map(|node| node.inner_html())
            } else if selector.starts_with(".") {
                // Class selector
                document.find(Class(selector.trim_start_matches(".")))
                    .next()
                    .map(|node| node.inner_html())
            } else {
                // Tag selector
                document.find(Name(selector))
                    .next()
                    .map(|node| node.inner_html())
            };
            
            if let Some(html) = content {
                return apply_template(&html, title);
            }
        }
        
        // Fallback: use the entire body
        let body = document.find(Name("body"))
            .next()
            .map(|node| node.inner_html())
            .unwrap_or_else(|| {
                // If no body tag is found, use the entire HTML content
                document.find(Name("html"))
                    .next()
                    .map(|node| node.inner_html())
                    .unwrap_or_else(|| "No content found".to_string())
            });
        
        apply_template(&body, title)
    }
}

pub fn replace_image_urls(html_content: &str, image_map: &HashMap<String, (String, Vec<u8>, &'static str)>) -> String {
    let img_regex = Regex::new(r#"(<img[^>]*src=["'])([^"']+)(["'][^>]*>)"#).unwrap();
    
    img_regex.replace_all(html_content, |caps: &regex::Captures| {
        let prefix = caps.get(1).unwrap().as_str();
        let img_src = caps.get(2).unwrap().as_str();
        let suffix = caps.get(3).unwrap().as_str();
        
        if let Some((local_path, _, _)) = image_map.get(img_src) {
            format!("{}{}{}", prefix, local_path, suffix)
        } else {
            // Keep original if not found in our map
            format!("{}{}{}", prefix, img_src, suffix)
        }
    }).to_string()
}

pub fn apply_template(content: &str, title: &str) -> Result<String> {
    // Use the embedded template
    let result = TEMPLATE_HTML
        .replace("{{TITLE}}", title)
        .replace("{{CONTENT}}", content);
    
    Ok(result)
}

