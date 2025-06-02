use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// URL of the website to convert to EPUB
    #[arg(short, long)]
    pub url: String,

    /// Output file path (default: website_title.epub)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub fn parse_args() -> Args {
    Args::parse()
}
