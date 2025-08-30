use crate::{AaxcDownloadConvertBase, DownloadOptions, mpeg_util};
use anyhow::Result;
use std::io::{Read, Seek, Write};

pub trait Converter<T: DownloadOptions> {
    fn convert_single(&self, in_stream: impl Read + Seek, out_stream: impl Write) -> Result<()>;
    fn convert_multi(&self, in_stream: impl Read + Seek, out_stream: impl Write) -> Result<()>;
}

pub struct AaxConverter<T: DownloadOptions> {
    pub base: AaxcDownloadConvertBase<T>,
}

impl<T: DownloadOptions> Converter<T> for AaxConverter<T> {
    fn convert_single(&self, mut in_stream: impl Read + Seek, mut out_stream: impl Write) -> Result<()> {
        let buffer = self.base.decrypt_to_buffer(&mut in_stream)?;
        let apple_tags = self.base.step_get_metadata(&buffer)?;
        let start_time = symphonia::core::units::Time::from(0u64);
        let end_time = symphonia::core::units::Time::from(u64::MAX);
        mpeg_util::encode_to_mp3(&buffer, &mut out_stream, &apple_tags, start_time, end_time)?;
        Ok(())
    }

    fn convert_multi(&self, mut in_stream: impl Read + Seek, _out_stream: impl Write) -> Result<()> {
        let buffer = self.base.decrypt_to_buffer(&mut in_stream)?;
        let apple_tags = self.base.step_get_metadata(&buffer)?;

        for (i, chapter) in apple_tags.chapters.chapters.iter().enumerate() {
            let next_chapter_start = if i + 1 < apple_tags.chapters.chapters.len() {
                apple_tags.chapters.chapters[i + 1].start_time
            } else {
                // This is not ideal, as we don't know the total duration.
                // For now, we'll just encode to the end of the buffer.
                symphonia::core::units::Time::from(u64::MAX)
            };

            let file_name = format!("{:02}_{}.mp3", i + 1, chapter.title.replace("/", "_"));
            let out_path = self.base._out_directory.join(file_name);
            let mut out_file = std::fs::File::create(out_path)?;

            mpeg_util::encode_to_mp3(&buffer, &mut out_file, &apple_tags, chapter.start_time, next_chapter_start)?;
        }

        Ok(())
    }
}
