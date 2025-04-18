# http-epub

A Rust command-line tool that converts websites to EPUB format.

## Features

- Convert any website to an EPUB file
- Automatically extracts title from the webpage
- Customizable output filename
- Intelligent content extraction from common website layouts
- Automatically downloads and includes images in the EPUB
- Always uses print-friendly or mobile layouts when available for cleaner content

## Installation

Ensure you have Rust and Cargo installed, then build the project:

```bash
cargo build --release
```

The binary will be available at `target/release/http-epub`.

## Usage

```bash
# Basic usage with just a URL
http-epub --url https://example.com

# Specify custom output file
http-epub --url https://example.com --output my-ebook.epub

# Set custom title
http-epub --url https://example.com --title "My Custom Title"
```

### Command-line Options

- `-u, --url <URL>`: URL of the website to convert (required)
- `-o, --output <FILE>`: Output file path (default: website_title.epub)
- `-t, --title <TITLE>`: Custom title for the EPUB (default: extracted from website)

## How It Works

1. Fetches the HTML content from the specified URL (always using print-friendly or mobile version when available)
2. Parses the HTML and extracts the main content
3. Downloads and processes all images found in the content
4. Creates an EPUB file with the extracted content and images
5. Adds metadata (title and fixed author "http-epub") to the EPUB
6. Saves the EPUB to the specified location

## Limitations

- Basic content extraction that may not work perfectly on all websites
- Limited handling of complex layouts or JavaScript-rendered content
- No support for fetching multiple pages or following links
- CSS styling from the original website is not preserved
- Image support is limited to standard formats (JPEG, PNG, GIF, SVG, WebP)

## Cross-Compilation for Android ARM64

This project can be cross-compiled for Android ARM64 devices using devbox. The setup process has been automated with scripts in the `bin` directory.

### Prerequisites

- Devbox installed on your system

### Setup

1. Run the setup script to download and install the Android NDK and cargo-ndk:

```bash
devbox run setup-android
```

This script will:
- Download and extract the Android NDK r26c to `~/Android/Sdk/ndk/`
- Install cargo-ndk if not already installed

### Building for Android ARM64

To build for ARM64 (aarch64):

```bash
devbox run build-android-arm64
```

The compiled binary will be available at:
`target/aarch64-linux-android/release/http-epub`


## License

MIT
