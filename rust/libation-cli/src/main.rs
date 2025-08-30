use clap::Parser;
use std::path::PathBuf;
use std::fs::File;
use aax_decrypter::{AaxcDownloadConvertBase, DownloadOptions, KeyData, FileType};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Input file
    #[clap(short, long, value_parser)]
    input: PathBuf,

    /// Output file
    #[clap(short, long, value_parser)]
    output: PathBuf,

    /// Activation bytes
    #[clap(short, long, value_parser)]
    activation_bytes: String,
}

struct CliDownloadOptions {
    keys: Vec<KeyData>,
}

impl DownloadOptions for CliDownloadOptions {
    fn decryption_keys(&self) -> Option<&[KeyData]> {
        Some(&self.keys)
    }
    fn input_type(&self) -> FileType {
        FileType::Aax
    }
    fn strip_unabridged(&self) -> bool { false }
    fn fixup_file(&self) -> bool { false }
    fn title(&self) -> &str { "" }
    fn subtitle(&self) -> Option<&str> { None }
    fn publisher(&self) -> Option<&str> { None }
    fn language(&self) -> Option<&str> { None }
    fn audible_product_id(&self) -> Option<&str> { None }
    fn series_name(&self) -> Option<&str> { None }
    fn series_number(&self) -> Option<&str> { None }
}


fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    let keys = vec![KeyData {
        key_part1: hex::decode(cli.activation_bytes)?,
        key_part2: None,
    }];
    let opts = CliDownloadOptions { keys };
    let converter = AaxcDownloadConvertBase::new("/tmp", "/tmp", opts);

    let mut in_file = File::open(cli.input)?;
    let mut out_file = File::create(cli.output)?;

    converter.decrypt_and_encode_to_mp3(&mut in_file, &mut out_file)?;

    Ok(())
}
