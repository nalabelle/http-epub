use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use epub_builder::{EpubBuilder, EpubContent, ReferenceType, ZipLibrary};
use std::fs::File;
use std::io::Cursor;
use std::path::PathBuf;
use tera::{Context as TeraContext, Tera}; // Add Tera imports

// Embed the template files directly into the binary
const TEMPLATE_HTML: &str = include_str!("template.html"); // For the main article content
const COVER_TEMPLATE_HTML: &str = include_str!("cover_template.html"); // For the cover page

use crate::extract::ExtractedContent;
use tracing::{debug, warn};

// Helper function to generate cover page XHTML using Tera
fn generate_cover_xhtml(
    tera: &Tera,
    extracted: &ExtractedContent,
    actual_cover_image_epub_path: Option<&str>,
) -> Result<String> {
    let mut context = TeraContext::new();
    context.insert("title", &extracted.title);
    if let Some(cover_path) = actual_cover_image_epub_path {
        context.insert("cover_image_local_path", cover_path);
    }
    // Tera's `if` handles missing variables gracefully, so no need to conditionally insert `author`
    // unless we want to ensure it's an empty string vs. not present.
    // For simplicity, only insert if meaningfully present.
    if !extracted.article_author.is_empty() && extracted.article_author != "http-epub" {
        // Check if author is meaningful
        context.insert("author", &extracted.article_author);
    }
    context.insert("original_url", extracted.original_url.as_str());
    context.insert(
        "original_url_domain",
        extracted
            .original_url
            .host_str()
            .unwrap_or_else(|| extracted.original_url.as_str()),
    );
    if let Some(date) = extracted.date_published {
        context.insert(
            "date_published_formatted",
            &date.format("%B %d, %Y at %l:%M %p").to_string(),
        );
    }

    // Add epubification date (current time when the EPUB is being created)
    let epubification_date = Utc::now();
    context.insert(
        "epubification_date_formatted",
        &epubification_date
            .format("%B %d, %Y at %l:%M %p")
            .to_string(),
    );

    tera.render("cover_template.html", &context)
        .map_err(|e| anyhow!("Failed to render cover template: {}", e))
}

// Helper function to apply the article template using Tera
fn apply_article_template(tera: &Tera, content_body: &str, title: &str) -> Result<String> {
    let mut context = TeraContext::new();
    context.insert("title", title);
    context.insert("content", content_body);

    tera.render("template.html", &context) // Assuming "template.html" is the article template name
        .map_err(|e| anyhow!("Failed to render article template: {}", e))
}

pub fn create_epub(
    extracted: &ExtractedContent,
    output_path_option: Option<&PathBuf>,
) -> Result<PathBuf> {
    // Renamed output_path
    // Initialize Tera and load templates by string content
    let mut tera = Tera::default();
    tera.add_raw_template("template.html", TEMPLATE_HTML) // TEMPLATE_HTML is the article template
        .context("Failed to add article template to Tera")?;
    tera.add_raw_template("cover_template.html", COVER_TEMPLATE_HTML)
        .context("Failed to add cover template to Tera")?;

    // Generate output path if not provided
    let mut final_path = match output_path_option {
        // Use renamed parameter
        Some(path) => path.clone(),
        None => {
            let filename = sanitize_filename::sanitize(format!("{}.epub", extracted.title));
            PathBuf::from(filename)
        }
    };

    // Check if the file exists and find an alternative name if it does
    if final_path.exists() {
        let mut counter = 1;
        let original_stem = final_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let extension = final_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        loop {
            let new_filename_str = if extension.is_empty() {
                format!("{} ({})", original_stem, counter)
            } else {
                format!("{} ({}).{}", original_stem, counter, extension)
            };
            let new_path = final_path.with_file_name(new_filename_str);
            if !new_path.exists() {
                final_path = new_path;
                break;
            }
            counter += 1;
        }
    }

    // Create a new EPUB
    let file = File::create(&final_path).context(format!(
        "Failed to create output file: {}",
        final_path.display()
    ))?;

    let zip_library =
        ZipLibrary::new().map_err(|e| anyhow!("Failed to create ZIP library: {}", e))?;
    let mut epub = EpubBuilder::new(zip_library)
        .map_err(|e| anyhow!("Failed to create EPUB builder: {}", e))?;

    // Set metadata
    epub.metadata("title", &extracted.title)
        .map_err(|e| anyhow!("Failed to set title metadata: {}", e))?;
    epub.metadata("author", &extracted.article_author)
        .map_err(|e| anyhow!("Failed to set author metadata: {}", e))?;
    // Use add_metadata_opf for dc:identifier with the URL
    epub.add_metadata_opf(epub_builder::MetadataOpf {
        name: "dc:identifier".to_string(),
        content: extracted.original_url.as_str().to_string(),
    });
    if let Some(date_published) = extracted.date_published {
        // Using add_metadata_opf for dc:date as per common EPUB practices
        epub.add_metadata_opf(epub_builder::MetadataOpf {
            name: "dc:date".to_string(),
            content: date_published.to_rfc3339(),
        });
        // epub_builder also has epub.date(timestamp) but it's for a specific OPF <meta property="dcterms:modified">
        // For publication date, dc:date is standard.
    }
    epub.set_modified_date(Utc::now()); // This sets <meta property="dcterms:modified">

    // Determine cover image details from original_thumbnail_url and image_map
    let mut cover_image_local_path: Option<String> = None;

    if let Some(ref original_thumb_url) = extracted.original_thumbnail_url {
        if let Some(downloaded_cover_info) = extracted.image_map.get(original_thumb_url.as_str()) {
            debug!(
                "Setting cover image using: {}",
                downloaded_cover_info.local_path
            );
            epub.add_cover_image(
                downloaded_cover_info.local_path.clone(),
                Cursor::new(downloaded_cover_info.data.clone()),
                downloaded_cover_info.mime_type,
            )
            .map_err(|e| {
                anyhow!(
                    "Failed to set cover image {}: {}",
                    downloaded_cover_info.local_path,
                    e
                )
            })?;
            cover_image_local_path = Some(downloaded_cover_info.local_path.clone());
        } else {
            warn!(
                "Original thumbnail URL was present but not found in image_map. No EPUB cover image set via add_cover_image."
            );
        }
    }

    // Generate and add the cover.xhtml page
    let cover_xhtml_content =
        generate_cover_xhtml(&tera, extracted, cover_image_local_path.as_deref())?;
    epub.add_content(
        EpubContent::new("cover.xhtml", cover_xhtml_content.as_bytes())
            .title("Cover")
            .reftype(ReferenceType::Cover),
    )
    .map_err(|e| anyhow!("Failed to add cover page content: {}", e))?;

    // Add all images from the map as resources.
    // The cover image (if set by add_cover_image) is already added as a resource by epub-builder.
    // We iterate here to add any other images.
    for (original_url_str, downloaded_image_info) in &extracted.image_map {
        // Check if this image was the one used as the EPUB cover.
        // If cover_image_local_path is Some and matches current image's local_path, it was the cover.
        let is_this_image_the_epub_cover = cover_image_local_path
            .as_ref()
            .is_some_and(|cover_path_val| {
                *cover_path_val == downloaded_image_info.local_path
            });

        if is_this_image_the_epub_cover {
            debug!(
                "Skipping re-adding EPUB cover image resource via add_resource: {}",
                downloaded_image_info.local_path
            );
            continue;
        }

        debug!(
            "Adding image resource: {}",
            downloaded_image_info.local_path
        );
        epub.add_resource(
            downloaded_image_info.local_path.clone(),
            Cursor::new(downloaded_image_info.data.clone()),
            downloaded_image_info.mime_type,
        )
        .map_err(|e| anyhow!("Failed to add image resource {}: {}", original_url_str, e))?;
    }

    // Apply template to the body content for the article page
    let article_xhtml_content =
        apply_article_template(&tera, &extracted.content, &extracted.title)?;

    // Add main content (article body)
    epub.add_content(
        EpubContent::new("article.xhtml", article_xhtml_content.as_bytes())
            .title(&extracted.title)
            .reftype(ReferenceType::Text),
    )
    .map_err(|e| anyhow!("Failed to add main article content: {}", e))?;

    // Generate EPUB
    epub.generate(
        file.try_clone()
            .context("Failed to clone file handle for EPUB generation")?,
    ) // Ensure file is cloneable or re-opened if needed by library
    .map_err(|e| anyhow!("Failed to generate EPUB: {}", e))?;

    Ok(final_path)
}
