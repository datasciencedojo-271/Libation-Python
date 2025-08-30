use crate::AppleTags;
use mp3lame_encoder::{Builder, Encoder, Bitrate, Quality, BuildError, DualPcm};
use anyhow::Result;
use std::io::Write;
use std::mem::MaybeUninit;

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
            56 => Bitrate::Kbps64,
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

pub fn encode_to_mp3(
    buffer: &[u8],
    mut out_stream: impl Write,
    apple_tags: &AppleTags,
) -> Result<()> {
    let chapters = ChapterInfo { count: 0 };
    let mut encoder = configure_lame_options(apple_tags, false, false, &chapters)?;

    let source = Box::new(std::io::Cursor::new(buffer.to_vec()));
    let mss = symphonia::core::io::MediaSourceStream::new(source, Default::default());
    let mut hint = symphonia::core::probe::Hint::new();
    hint.with_extension("m4b");
    let probed = symphonia::default::get_probe().format(&hint, mss, &Default::default(), &Default::default())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| anyhow::anyhow!("No default track found"))?;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &Default::default())?;

    let mut mp3_buffer = [MaybeUninit::new(0); 1024 * 1024];

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                continue;
            }
            Err(_) => {
                continue;
            }
        };

        let decoded = decoder.decode(&packet)?;
        let mut sample_buf = symphonia::core::audio::SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
        sample_buf.copy_interleaved_ref(decoded);
        let samples = sample_buf.samples();
        let left: Vec<i16> = samples.iter().step_by(2).cloned().collect();
        let right: Vec<i16> = samples.iter().skip(1).step_by(2).cloned().collect();
        let size = encoder.encode(DualPcm { left: &left, right: &right }, &mut mp3_buffer).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        out_stream.write_all(unsafe { &*(&mp3_buffer[..size] as *const [MaybeUninit<u8>] as *const [u8]) })?;
    }

    let size = encoder.flush::<mp3lame_encoder::FlushNoGap>(&mut mp3_buffer).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    out_stream.write_all(unsafe { &*(&mp3_buffer[..size] as *const [MaybeUninit<u8>] as *const [u8]) })?;

    Ok(())
}
