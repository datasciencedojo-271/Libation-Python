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

    /// Move moov atom to beginning of file
    #[clap(long)]
    move_moov: bool,
}

struct CliDownloadOptions {
    keys: Vec<KeyData>,
    move_moov: bool,
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
    fn create_cue_sheet(&self) -> bool { false }
    fn lame_config(&self) -> &aax_decrypter::mpeg_util::LameConfig {
        &aax_decrypter::mpeg_util::LameConfig {
            vbr: None,
            abr_rate_kbps: 128,
            bitrate: 128,
        }
    }
    fn move_moov_to_beginning(&self) -> bool { self.move_moov }
}


fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    let keys = vec![KeyData {
        key_part1: hex::decode(cli.activation_bytes)?,
        key_part2: None,
    }];
    let progress_bar = indicatif::ProgressBar::new(100);
    progress_bar.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"),
    );

    let opts = CliDownloadOptions { keys: keys.clone(), move_moov: cli.move_moov };
    let converter = aax_decrypter::converter::AaxConverter {
        base: AaxcDownloadConvertBase::new(cli.output.parent().unwrap_or(&PathBuf::from(".")), &cli.output, opts)
            .with_progress_callback(Box::new({
                let progress_bar = progress_bar.clone();
                move |p| {
                    progress_bar.set_position(p.percentage as u64);
                    progress_bar.set_message(p.message);
                }
            })),
    };

    let mut in_file = File::open(cli.input)?;

    if cli.multi {
        // The output path is a directory for multi-file conversion
        std::fs::create_dir_all(&cli.output)?;
        let _ = converter.convert_multi(&mut in_file, std::io::sink())?;
    } else {
        let mut out_file = File::create(cli.output)?;
        converter.convert_single(&mut in_file, &mut out_file)?;
    }

    progress_bar.finish_with_message("Done!");

    Ok(())
}
