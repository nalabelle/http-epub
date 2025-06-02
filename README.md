# http-epub

A Rust command-line tool that converts websites to EPUB format.

## Features

- Convert any website to an EPUB file
- Automatically extracts title from the webpage
- Customizable output filename
- Intelligent content extraction from common website layouts
- Automatically downloads and includes images in the EPUB
- Always uses print-friendly or mobile layouts when available for cleaner content

## Usage

```bash
# Basic usage with just a URL
http-epub --url https://example.com

# Specify custom output file
http-epub --url https://example.com --output my-ebook.epub
```

### Command-line Options

- `-u, --url <URL>`: URL of the website to convert (required)
- `-o, --output <FILE>`: Output file path (default: website_title.epub)

## Limitations

- Basic content extraction that may not work perfectly on all websites
- Limited handling of complex layouts or JavaScript-rendered content
- No support for fetching multiple pages or following links
- CSS styling from the original website is not preserved
- Image support is limited to standard formats (JPEG, PNG, GIF, SVG, WebP)
