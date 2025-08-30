use clap::Parser;
use std::path::PathBuf;
use std::fs::File;
use aax_decrypter::{AaxcDownloadConvertBase, DownloadOptions, KeyData, FileType, converter::Converter};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Input file
    #[clap(short, long, value_parser)]
    input: PathBuf,

    /// Output file or directory
    #[clap(short, long, value_parser)]
    output: PathBuf,

    /// Activation bytes
    #[clap(short, long, value_parser)]
    activation_bytes: String,

    /// Split by chapters
    #[clap(long)]
    multi: bool,
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
    let converter = aax_decrypter::converter::AaxConverter {
        base: AaxcDownloadConvertBase::new(cli.output.parent().unwrap_or(&PathBuf::from(".")), &cli.output, opts),
    };

    let mut in_file = File::open(cli.input)?;

    if cli.multi {
        // The output path is a directory for multi-file conversion
        let _ = converter.convert_multi(&mut in_file, std::io::sink())?;
    } else {
        let mut out_file = File::create(cli.output)?;
        converter.convert_single(&mut in_file, &mut out_file)?;
    }

    Ok(())
}
