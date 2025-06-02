use anyhow::Result;

// cli module is local to the binary
mod cli;
// epub, extract, fetch are part of the library (lib.rs) and accessed via http_epub::

fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = cli::parse_args();

    // Call the library function to handle the core logic.
    // The crate name is 'http-epub', so in code it's 'http_epub'.
    println!("Processing URL: {}", args.url);
    let output_path = http_epub::url_to_epub(&args.url, args.output.as_ref())?;

    println!("EPUB successfully created at: {}", output_path.display());
    Ok(())
}
