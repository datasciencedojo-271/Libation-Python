use crate::AppleTags;
use mp3lame_encoder::{Builder, Encoder, Bitrate, Quality, BuildError};

// This is a placeholder for the chapter info that is used in the C# code
pub struct ChapterInfo {
    pub count: usize,
}

pub fn configure_lame_options(
    _apple_tags: &AppleTags, // In the future, this will be a more complete representation of the mp4 file
    downsample: bool,
    match_source_bitrate: bool,
    _chapters: &ChapterInfo,
) -> Result<Encoder, anyhow::Error> {
    let mut builder = Builder::new().ok_or_else(|| anyhow::anyhow!("Failed to create LAME builder"))?;

    // In the C# code, these are derived from the mp4 file's properties
    let source_samplerate = 44100;
    let source_channels = 2;
    let source_bitrate = 128000;

    builder.set_sample_rate(source_samplerate).map_err(|e: BuildError| anyhow::anyhow!(e.to_string()))?;
    builder.set_num_channels(source_channels).map_err(|e: BuildError| anyhow::anyhow!(e.to_string()))?;

    if downsample && source_channels == 2 {
        builder.set_num_channels(1).map_err(|e: BuildError| anyhow::anyhow!(e.to_string()))?;
    }

    if match_source_bitrate {
        let bitrate = match source_bitrate / 1000 {
            320 => Bitrate::Kbps320,
            256 => Bitrate::Kbps256,
            224 => Bitrate::Kbps224,
            192 => Bitrate::Kbps192,
            160 => Bitrate::Kbps160,
            128 => Bitrate::Kbps128,
            112 => Bitrate::Kbps112,
            96 => Bitrate::Kbps96,
            80 => Bitrate::Kbps80,
            64 => Bitrate::Kbps64,
            56 => Bitrate::Kbps64, // Corrected typo
            48 => Bitrate::Kbps48,
            40 => Bitrate::Kbps40,
            32 => Bitrate::Kbps32,
            _ => Bitrate::Kbps128, // Default
        };
        builder.set_brate(bitrate).map_err(|e: BuildError| anyhow::anyhow!(e.to_string()))?;
    }

    builder.set_quality(Quality::Best).map_err(|e: BuildError| anyhow::anyhow!(e.to_string()))?;

    // TODO: Set ID3 tags from apple_tags
    // The `mp3lame-encoder` crate has some support for ID3 tags, but it might not be as extensive as the C# code.
    // I will need to investigate this further.

    let encoder = builder.build().map_err(|e: BuildError| anyhow::anyhow!(e.to_string()))?;

    Ok(encoder)
}
