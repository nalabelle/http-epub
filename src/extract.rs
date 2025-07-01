use crate::fetch::{DownloadedImage, FetchedContent, Fetcher};
use ammonia::Builder;
use anyhow::Result;
use article_extractor::{Article, FullTextParser};
use chrono::{DateTime, Utc};
use dom_query::Document as DomDocument;
use maplit::{hashmap, hashset};
use std::collections::{HashMap, HashSet};
use tracing::{debug, instrument, warn};
use url::Url;

pub struct ExtractedContent {
    pub content: String,
    pub image_map: HashMap<String, DownloadedImage>,
    pub title: String,
    pub original_url: Url,
    pub article_author: String,
    pub date_published: Option<DateTime<Utc>>,
    pub original_thumbnail_url: Option<Url>,
}

pub struct ParsedArticle {
    pub article: Article,
    pub document: DomDocument,
    pub head_document: DomDocument,
}

pub struct Extractor {
    fetcher: Fetcher,
    parser: FullTextParser,
}

impl Default for Extractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor {
    pub fn new() -> Self {
        Self {
            fetcher: Fetcher::new(),
            parser: FullTextParser::new(None),
        }
    }
    #[instrument(skip(self), fields(original_url))]
    pub fn process(&self, original_url: &Url) -> Result<ExtractedContent> {
        let content = self.fetcher.fetch_content(original_url)?; // `content` is FetchedContent
        let parsed = self.parsed_article(content.clone())?; // `parsed` is ParsedArticle
        let mut image_urls = self.extract_image_urls(&parsed);

        // Determine the absolute thumbnail URL if it exists
        let absolute_thumbnail_url: Option<Url> =
            parsed.article.thumbnail_url.as_ref()
            .and_then(|thumb_url_str| {
                match content.url.join(thumb_url_str) { // Use content.url as base
                    Ok(abs_url) => Some(abs_url),
                    Err(e) => {
                        warn!(url = thumb_url_str, error = %e, "Failed to resolve thumbnail URL to absolute path");
                        None
                    }
                }
            });

        // Add absolute thumbnail URL to the set of required URLs to download
        if let Some(ref abs_thumb_url) = absolute_thumbnail_url {
            image_urls.insert(abs_thumb_url.clone());
        }

        let image_map = self.fetcher.download_image_list(&image_urls)?; // image_map keys are absolute URL strings

        // Clean HTML first, then process DOM transformations directly
        let body_html = self.extract_body(&parsed);
        let cleaned_body_html = self.clean_html(body_html);

        // Create a new document from cleaned HTML for further processing
        let mut cleaned_document = DomDocument::from(cleaned_body_html);
        self.convert_video_tags_to_links(&mut cleaned_document, &content.url);
        self.replace_image_urls(&mut cleaned_document, &image_map, &content.url);

        let final_body = cleaned_document.html().to_string();
        let title = self.extract_title(&parsed);
        let article_author = self.extract_author(&parsed);
        let date_published = parsed.article.date.or_else(|| {
            debug!("No date found in article_extractor, trying meta tags...");
            self._extract_date_from_meta_tags(&parsed.head_document)
        });

        if let Some(ref date) = date_published {
            debug!("Final extracted date: {}", date);
        } else {
            debug!("No date could be extracted.");
        }

        Ok(ExtractedContent {
            content: final_body,
            image_map,
            title,
            original_url: original_url.clone(),
            article_author,
            date_published,
            original_thumbnail_url: absolute_thumbnail_url,
        })
    }

    #[instrument(skip_all)]
    fn parsed_article(&self, content: FetchedContent) -> Result<ParsedArticle> {
        // Extract just the head element content from the original HTML before moving content.html_string
        let original_document = DomDocument::from(content.html_string.as_str());
        let head_html = if let Some(head_element) = original_document.select("head").nodes().first()
        {
            head_element.inner_html().to_string()
        } else {
            // Fallback: empty head content
            String::new()
        };
        let head_document = DomDocument::from(head_html.as_str());

        let article_product =
            self.parser
                .parse_offline(vec![content.html_string], None, Some(content.url))?;

        // Get the HTML string for Document parsing.
        // If article_product.html is None, return an error.
        let html_for_document_str: String;
        if let Some(html_ref) = article_product.html.as_deref() {
            html_for_document_str = html_ref.to_string();
        } else {
            return Err(anyhow::anyhow!(
                "Article content (HTML) is None after parsing by article_extractor"
            ));
        }
        Ok(ParsedArticle {
            article: article_product,
            document: DomDocument::from(html_for_document_str.as_str()),
            head_document,
        })
    }

    fn extract_author(&self, parsed: &ParsedArticle) -> String {
        if let Some(author) = &parsed.article.author {
            if !author.trim().is_empty() {
                debug!("Found author via article_extractor: {}", author);
                return author.clone();
            }
        }
        debug!(
            "No author found via article_extractor or author was empty, trying meta tag fallback..."
        );
        // Fallback to checking meta tags
        let author_meta_selectors = [
            "meta[name=\"author\"]",
            "meta[property=\"article:author\"]",
            "meta[name=\"dc.creator\"]",
            "meta[name=\"dcterms.creator\"]",
            "meta[property=\"author\"]",
        ];

        for selector in author_meta_selectors.iter() {
            let author_selection = parsed.head_document.select(selector);
            if let Some(element) = author_selection.nodes().first() {
                if let Some(content) = element.attr("content") {
                    let content_str = content.to_string();
                    if !content_str.trim().is_empty() {
                        debug!("Found author via meta tag {}: {}", selector, content_str);
                        return content_str;
                    }
                }
            }
        }
        debug!("No author found in meta tags or content was empty, using default fallback.");
        "http-epub".to_string()
    }

    fn _extract_date_from_meta_tags(&self, document: &DomDocument) -> Option<DateTime<Utc>> {
        let meta_selectors = [
            "meta[property=\"article:published_time\"]",
            "meta[name=\"publish-date\"]",
            "meta[name=\"date\"]", // Common alternative
        ];
        for selector in meta_selectors.iter() {
            let date_selection = document.select(selector);
            if let Some(element) = date_selection.nodes().first() {
                if let Some(content) = element.attr("content") {
                    let content_str = content.to_string();
                    if !content_str.trim().is_empty() {
                        debug!(tag = selector, content = content_str, "Found date meta tag");
                        // Attempt RFC3339 first
                        if let Ok(dt) = DateTime::parse_from_rfc3339(&content_str) {
                            return Some(dt.with_timezone(&Utc));
                        }
                        // Attempt custom formats
                        let formats_to_try = [
                            "%Y-%m-%dT%H:%M:%S%z",     // Full ISO with timezone
                            "%Y-%m-%dT%H:%M:%S%.3f%z", // ISO with milliseconds
                            "%Y-%m-%d %H:%M:%S %z",
                            "%b %d, %Y %I:%M %p", // "May 31, 2025 10:11 AM"
                            "%B %d, %Y %I:%M %p", // "May 31, 2025 10:11 AM"
                            "%Y-%m-%d",           // Date only
                        ];
                        for fmt in formats_to_try.iter() {
                            if let Ok(naive_dt) =
                                chrono::NaiveDateTime::parse_from_str(&content_str, fmt)
                            {
                                debug!(format = fmt, "Successfully parsed date with custom format");
                                return Some(DateTime::<Utc>::from_naive_utc_and_offset(
                                    naive_dt, Utc,
                                ));
                            }
                            if let Ok(naive_date) =
                                chrono::NaiveDate::parse_from_str(&content_str, fmt)
                            {
                                if let Some(dt_at_midnight) = naive_date.and_hms_opt(0, 0, 0) {
                                    debug!(
                                        format = fmt,
                                        "Successfully parsed date-only with custom format"
                                    );
                                    return Some(DateTime::<Utc>::from_naive_utc_and_offset(
                                        dt_at_midnight,
                                        Utc,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_title(&self, parsed: &ParsedArticle) -> String {
        if let Some(title) = &parsed.article.title {
            return title.clone();
        }
        "Unknown".to_string()
    }

    #[instrument(skip_all)]
    fn extract_image_urls(&self, parsed: &ParsedArticle) -> HashSet<Url> {
        let mut string_urls_to_resolve = HashSet::new();

        let img_selection = parsed.document.select("img");
        for img_node in img_selection.nodes().iter() {
            if let Some(src_str) = img_node.attr("src") {
                if !src_str.starts_with("data:") {
                    string_urls_to_resolve.insert(src_str.to_string());
                }
            }
        }

        let mut resolved_urls = HashSet::new();
        for url_s in string_urls_to_resolve {
            match parsed.article.url.join(&url_s) {
                // Now uses the passed page_base_url
                Ok(abs_url) => {
                    resolved_urls.insert(abs_url);
                }
                Err(e) => {
                    warn!(src = %url_s, base = %parsed.article.url, error = %e, "Failed to parse/resolve image URL");
                }
            }
        }
        debug!(
            count = resolved_urls.len(),
            "Identified and resolved image URLs from HTML content."
        );
        resolved_urls
    }

    #[instrument(skip_all)]
    fn convert_video_tags_to_links(&self, document: &mut DomDocument, page_base_url: &Url) {
        // Find all video elements and replace them with links
        let video_selection = document.select("video");

        // Collect video elements to avoid borrowing issues
        let mut replacements = Vec::new();

        for video_element in video_selection.nodes().iter() {
            let mut video_url = None;

            // First try to get src from video tag
            if let Some(src_str) = video_element.attr("src") {
                video_url = Some(src_str.to_string());
            } else {
                // Look for source tags inside the video
                for child in video_element.children() {
                    if child.node_name().as_deref() == Some("source") {
                        if let Some(src_str) = child.attr("src") {
                            video_url = Some(src_str.to_string());
                            break; // Use the first source found
                        }
                    }
                }
            }

            let replacement_html = if let Some(url_str) = video_url {
                // Resolve the URL against the page's base URL
                match page_base_url.join(&url_str) {
                    Ok(abs_url) => {
                        format!(
                            r#"<p><a href="{abs_url}" title="Video content">🎥 Watch Video: {abs_url}</a></p>"#
                        )
                    }
                    Err(e) => {
                        warn!(src = url_str, base = %page_base_url, error = %e, "Failed to resolve video URL");
                        format!(
                            r#"<p><a href="{url_str}" title="Video content">🎥 Watch Video: {url_str}</a></p>"#
                        )
                    }
                }
            } else {
                // No video source found, create a generic placeholder
                "<p><em>Video content not available</em></p>".to_string()
            };

            replacements.push((video_element.clone(), replacement_html));
        }

        // Apply replacements
        for (video_element, replacement_html) in replacements {
            video_element.replace_with_html(replacement_html);
        }
    }

    #[instrument(skip_all)]
    fn extract_body(&self, parsed: &ParsedArticle) -> String {
        let body_selection = parsed.document.select("body");
        if let Some(body_node) = body_selection.nodes().first() {
            return body_node.inner_html().to_string();
        }
        // If no body tag is found, article_extractor might have returned a fragment.
        // In this case, the 'document' field of ParsedArticle contains the full fragment.
        // We return its string representation.
        warn!(
            "No <body> tag found in parsed document content; using original article HTML as body."
        );
        // Fallback to the original HTML string stored in parsed.article.html.
        // This is safe because parsed_article ensures article.html is Some.
        parsed.article.html.as_deref().unwrap_or("").to_string()
    }

    #[instrument(skip_all)]
    fn clean_html(&self, article_html: String) -> String {
        // Use ammonia to clean and sanitize HTML content
        // Configure ammonia to allow common article elements but remove unwanted wrapper tags
        let cleaned = Builder::default()
            .tags(hashset![
                "p",
                "br",
                "strong",
                "b",
                "em",
                "i",
                "u",
                "h1",
                "h2",
                "h3",
                "h4",
                "h5",
                "h6",
                "ul",
                "ol",
                "li",
                "blockquote",
                "pre",
                "code",
                "a",
                "img",
                "figure",
                "figcaption",
                "table",
                "thead",
                "tbody",
                "tr",
                "td",
                "th",
                "span",
                "video",
                "source",
            ])
            .tag_attributes(hashmap![
                "a" => hashset!["href", "title"],
                "img" => hashset!["src", "alt", "title", "width", "height"],
                "blockquote" => hashset!["cite"],
                "table" => hashset!["summary"],
                "td" => hashset!["colspan", "rowspan"],
                "th" => hashset!["colspan", "rowspan", "scope"],
                "video" => hashset!["src", "controls", "width", "height", "poster"],
                "source" => hashset!["src", "type"]
            ])
            .url_schemes(hashset!["http", "https", "mailto"])
            .link_rel(None)
            .clean(&article_html)
            .to_string();

        // Replace &nbsp; with numeric entity for better EPUB compatibility
        let final_cleaned = cleaned.replace("&nbsp;", "&#160;");

        // Trim any leading/trailing whitespace
        final_cleaned.trim().to_string()
    }

    #[instrument(skip_all)]
    fn replace_image_urls(
        &self,
        document: &mut DomDocument,
        image_map: &HashMap<String, DownloadedImage>, // Key is absolute URL String
        page_base_url: &Url,                          // For resolving src attributes in HTML
    ) {
        let img_selection = document.select("img");

        for img_element in img_selection.nodes().iter() {
            if let Some(src_attr_val) = img_element.attr("src") {
                if src_attr_val.starts_with("data:") {
                    // Skip data URIs
                    continue;
                }

                // Resolve the src attribute value against the page's base URL
                match page_base_url.join(&src_attr_val) {
                    Ok(abs_url_from_html) => {
                        // Lookup this absolute URL string in our map
                        if let Some(downloaded_image_info) =
                            image_map.get(abs_url_from_html.as_str())
                        {
                            img_element.set_attr("src", &downloaded_image_info.local_path);
                        } else {
                            warn!(original_src = %src_attr_val, resolved_url = %abs_url_from_html, "Image src not found in map during replacement. Keeping original src.");
                        }
                    }
                    Err(e) => {
                        warn!(src = %src_attr_val, base = %page_base_url, error = %e, "Failed to resolve image src in replace_image_urls. Keeping original.");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_video_tag_conversion() {
        let extractor = Extractor::new();
        let base_url = Url::parse("https://example.com/page").unwrap();

        // Test video tag with src attribute
        let html_with_video = r#"<p>Some content</p><video src="video.mp4" controls>Your browser does not support the video tag.</video><p>More content</p>"#;
        let mut document = DomDocument::from(html_with_video);
        extractor.convert_video_tags_to_links(&mut document, &base_url);
        let result = document.html().to_string();
        assert!(result.contains("🎥 Watch Video: https://example.com/video.mp4"));
        assert!(result.contains(r#"href="https://example.com/video.mp4""#));

        // Test video tag with source child element
        let html_with_source = r#"<video controls><source src="movie.mp4" type="video/mp4">Your browser does not support the video tag.</video>"#;
        let mut document2 = DomDocument::from(html_with_source);
        extractor.convert_video_tags_to_links(&mut document2, &base_url);
        let result2 = document2.html().to_string();
        assert!(result2.contains("🎥 Watch Video: https://example.com/movie.mp4"));

        // Test video tag without source
        let html_no_source =
            r#"<video controls>Your browser does not support the video tag.</video>"#;
        let mut document3 = DomDocument::from(html_no_source);
        extractor.convert_video_tags_to_links(&mut document3, &base_url);
        let result3 = document3.html().to_string();
        assert!(result3.contains("Video content not available"));
    }
}
