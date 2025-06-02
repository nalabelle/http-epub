use crate::extract::Extractor;
use anyhow::Result;
use std::path::PathBuf;
use url::Url;

// Re-export modules
pub mod cli;
pub mod epub;
pub mod extract;
pub mod fetch;

/// Convert a URL to EPUB format and save to a file
pub fn url_to_epub(url_str: &str, output_path: Option<&PathBuf>) -> Result<PathBuf> {
    // Returns the path where the EPUB was saved
    // This function handles the entire process and saves to a file

    let url = Url::parse(url_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse input URL '{}': {}", url_str, e))?;

    let extractor = Extractor::new();

    // Process document - extract content, handle images, etc.
    let extracted_content = extractor.process(&url)?;

    // Create EPUB and save to file
    let final_output_path = epub::create_epub(&extracted_content, output_path)?;

    // Return the path where the EPUB was saved
    Ok(final_output_path)
}
