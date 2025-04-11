use anyhow::Result;
use std::path::PathBuf;

// Re-export modules
pub mod cli;
pub mod extract;
pub mod epub;
pub mod fetch;

/// Convert a URL to EPUB format and save to a file
pub fn url_to_epub(
    url: &str,
    output_path: Option<&PathBuf>,
    title: Option<&str>
) -> Result<PathBuf> {
    // Returns the path where the EPUB was saved
    // This function handles the entire process and saves to a file
    
    // Create fetcher and fetch content
    let fetcher = fetch::Fetcher::new();
    let fetched = fetcher.fetch_content(url)?;
    
    // Create extractor
    let extractor = extract::Extractor::new(&fetched);
    
    // Process document - extract content, handle images, and get title
    let title_str = title.map(|s| s.to_string());
    let extracted = extractor.process(title_str.as_ref())?;
    
    // Create EPUB and save to file
    let output_path = epub::create_epub(&extracted, "http-epub", output_path)?;
    
    // Return the path where the EPUB was saved
    Ok(output_path)
}


