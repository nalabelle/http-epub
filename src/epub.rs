use anyhow::{Context, Result, anyhow};
use epub_builder::{EpubBuilder, EpubContent, ZipLibrary, ReferenceType};
use std::fs::File;
use std::io::Cursor;
use std::path::PathBuf;

use crate::extract::ExtractedContent;

pub fn create_epub(
    extracted: &ExtractedContent,
    author: &str, 
    output_path: Option<&PathBuf>
) -> Result<PathBuf> {
    // Generate output path if not provided
    let final_path = match output_path {
        Some(path) => path.clone(),
        None => {
            let filename = sanitize_filename::sanitize(format!("{}.epub", extracted.title));
            PathBuf::from(filename)
        }
    };
    // Create a new EPUB
    let file = File::create(&final_path)
        .context("Failed to create output file")?;
    
    let zip_library = ZipLibrary::new().map_err(|e| anyhow!("Failed to create ZIP library: {}", e))?;
    let mut epub = EpubBuilder::new(zip_library).map_err(|e| anyhow!("Failed to create EPUB builder: {}", e))?;
    
    // Set metadata
    epub.metadata("title", &extracted.title).map_err(|e| anyhow!("Failed to set title metadata: {}", e))?;
    epub.metadata("author", author).map_err(|e| anyhow!("Failed to set author metadata: {}", e))?;
    
    // Use the already processed content
    
    // Add images to EPUB
    for (_img_url, (img_path, img_data, mime_type)) in &extracted.image_map {
        println!("Adding image: {}", img_path);
        epub.add_resource(
            img_path, 
            Cursor::new(img_data), 
            mime_type.to_string()
        ).map_err(|e| anyhow!("Failed to add image {}: {}", img_path, e))?;
    }
    
    // Add main content
    epub.add_content(
        EpubContent::new("main.xhtml", extracted.content.as_bytes())
            .title(&extracted.title)
            .reftype(ReferenceType::Text)
    ).map_err(|e| anyhow!("Failed to add content: {}", e))?;
    
    // Generate EPUB
    epub.generate(file).map_err(|e| anyhow!("Failed to generate EPUB: {}", e))?;
    
    Ok(final_path)
}
