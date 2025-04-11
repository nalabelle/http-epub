use anyhow::{Context, Result};

// Create modules
mod cli;
mod extract;
mod epub;
mod fetch;

fn main() -> Result<()> {
    // Parse command line arguments
    let args = cli::parse_args();
    
    // Create fetcher and fetch content
    let fetcher = fetch::Fetcher::new();
    let fetched = fetcher.fetch_content(&args.url)?;
    
    // Create extractor
    let extractor = extract::Extractor::new(&fetched);
    
    // Process document - extract content, handle images, and get title
    let extracted = extractor.process(args.title.as_ref())
        .context("Failed to process document")?;
    
    // Create EPUB
    println!("Creating EPUB...");
    let output_path = epub::create_epub(&extracted, "http-epub", args.output.as_ref())
        .context("Failed to create EPUB")?;
    
    println!("EPUB successfully created at: {}", output_path.display());
    Ok(())
}
















