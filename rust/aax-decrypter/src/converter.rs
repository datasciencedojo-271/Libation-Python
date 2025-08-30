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
        mpeg_util::encode_to_mp3(&buffer, &mut out_stream, &apple_tags)?;
        Ok(())
    }

    fn convert_multi(&self, _in_stream: impl Read + Seek, _out_stream: impl Write) -> Result<()> {
        // TODO: Implement multi-file conversion
        unimplemented!();
    }
}
