use std::io::{Read, Seek, Write, SeekFrom};
use anyhow::Result;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use mp3lame_encoder::{DualPcm, EncodeError};
use std::mem::MaybeUninit;

mod adrm_key_derivation;
mod mpeg_util;
mod atom;

use atom::{Atom, AavdAtom};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

pub struct AppleTags {
    // Add fields based on what's used in the C# code
    pub title: String,
    pub title_sans_unabridged: String,
    pub album: Option<String>,
    pub narrator: Option<String>,
    pub copyright: Option<String>,
    pub cover: Option<Vec<u8>>,
    pub asin: Option<String>,
    // This was a complex object in C#, for now a placeholder
    pub apple_list_box: AppleListBox,
}

// Placeholder for the complex AppleListBox
pub struct AppleListBox;

impl AppleListBox {
    pub fn edit_or_add_tag(&mut self, _tag: &str, _value: &str) {
        // TODO
    }

    pub fn edit_or_add_freeform_tag(&mut self, _domain: &str, _name: &str, _value: &str) {
        // TODO
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Dash,
    Aax,
    Aaxc,
}

pub struct KeyData {
    pub key_part1: Vec<u8>,
    pub key_part2: Option<Vec<u8>>,
}

pub trait DownloadOptions {
    fn decryption_keys(&self) -> Option<&[KeyData]>;
    fn input_type(&self) -> FileType;
    fn strip_unabridged(&self) -> bool;
    fn fixup_file(&self) -> bool;
    fn title(&self) -> &str;
    fn subtitle(&self) -> Option<&str>;
    fn publisher(&self) -> Option<&str>;
    fn language(&self) -> Option<&str>;
    fn audible_product_id(&self) -> Option<&str>;
    fn series_name(&self) -> Option<&str>;
    fn series_number(&self) -> Option<&str>;
}

// This will be a wrapper around the mp4 crate's structs
pub struct Mp4File {
    pub apple_tags: AppleTags,
}

pub struct AaxcDownloadConvertBase<T: DownloadOptions> {
    _out_directory: std::path::PathBuf,
    _cache_directory: std::path::PathBuf,
    dl_options: T,
    _cover_art: Option<Vec<u8>>,
    is_canceled: bool,
}

impl<T: DownloadOptions> AaxcDownloadConvertBase<T> {
    pub fn new(out_directory: impl AsRef<std::path::Path>, cache_directory: impl AsRef<std::path::Path>, dl_options: T) -> Self {
        Self {
            _out_directory: out_directory.as_ref().to_path_buf(),
            _cache_directory: cache_directory.as_ref().to_path_buf(),
            dl_options,
            _cover_art: None,
            is_canceled: false,
        }
    }

    pub fn set_cover_art(&mut self, cover_art: Vec<u8>) {
        self._cover_art = Some(cover_art);
    }

    pub async fn cancel(&mut self) {
        self.is_canceled = true;
    }

    pub fn decrypt_to_buffer(&self, mut in_stream: impl Read + Seek) -> Result<Vec<u8>> {
        let (key, iv) = match self.dl_options.input_type() {
            FileType::Aax => {
                let keys = self.dl_options.decryption_keys().ok_or_else(|| anyhow::anyhow!("Decryption keys cannot be null or empty for AAX"))?;
                let activation_bytes = hex::encode(&keys[0].key_part1);
                adrm_key_derivation::derive_key_iv(&mut in_stream, &activation_bytes)?
            }
            FileType::Aaxc => {
                let keys = self.dl_options.decryption_keys().ok_or_else(|| anyhow::anyhow!("Decryption keys cannot be null or empty for AAXC"))?;
                let key = keys[0].key_part1.clone();
                let iv = keys[0].key_part2.as_ref().cloned().ok_or_else(|| anyhow::anyhow!("IV cannot be null for Aaxc"))?;
                (key, iv)
            }
            FileType::Dash => {
                return Err(anyhow::anyhow!("DASH format not yet supported"));
            }
        };

        let mut out_buffer = Vec::new();
        process_file(&mut in_stream, &mut out_buffer, &key, &iv)?;
        Ok(out_buffer)
    }

    pub fn decrypt_and_encode_to_mp3(&self, mut in_stream: impl Read + Seek, mut out_stream: impl Write) -> Result<()> {
        let buffer = self.decrypt_to_buffer(&mut in_stream)?;

        // TODO: This is where metadata would be parsed and used to configure the encoder
        let apple_tags = AppleTags {
            title: "dummy title".to_string(),
            title_sans_unabridged: "dummy title".to_string(),
            album: None,
            narrator: None,
            copyright: None,
            cover: None,
            asin: None,
            apple_list_box: AppleListBox,
        };
        let chapters = mpeg_util::ChapterInfo { count: 0 };
        let mut encoder = mpeg_util::configure_lame_options(&apple_tags, false, false, &chapters)?;

        let source = Box::new(std::io::Cursor::new(buffer));
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
                    // The track list has been changed. Re-probe for the new track.
                    continue;
                }
                Err(_) => {
                    // A recoverable error occurred, continue reading the next packet.
                    continue;
                }
            };

            let decoded = decoder.decode(&packet)?;
            let mut sample_buf = symphonia::core::audio::SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
            sample_buf.copy_interleaved_ref(decoded);
            let samples = sample_buf.samples();
            let left: Vec<i16> = samples.iter().step_by(2).cloned().collect();
            let right: Vec<i16> = samples.iter().skip(1).step_by(2).cloned().collect();
            let size = encoder.encode(DualPcm { left: &left, right: &right }, &mut mp3_buffer).map_err(|e: EncodeError| anyhow::anyhow!(e.to_string()))?;
            out_stream.write_all(unsafe { &*(&mp3_buffer[..size] as *const [MaybeUninit<u8>] as *const [u8]) })?;
        }

        let size = encoder.flush::<mp3lame_encoder::FlushNoGap>(&mut mp3_buffer).map_err(|e: EncodeError| anyhow::anyhow!(e.to_string()))?;
        out_stream.write_all(unsafe { &*(&mp3_buffer[..size] as *const [MaybeUninit<u8>] as *const [u8]) })?;

        Ok(())
    }
}

const FTYP_ATOM_TYPE: u32 = 0x66747970;
const MDAT_ATOM_TYPE: u32 = 0x6d646174;
const AAVD_ATOM_TYPE: u32 = 0x61617664;
const MP4A_ATOM_TYPE: u32 = 0x6d703461;

const FTYP_TAGS: [u32; 6] = [
    0x4D344120, // M4A
    0x00000200, // VERSION2_0
    0x69736F32, // ISO2
    0x4D344220, // M4B
    0x6D703432, // MP42
    0x69736F6D, // ISOM
];

pub fn process_file(in_stream: &mut (impl Read + Seek), out_stream: &mut impl Write, key: &[u8], iv: &[u8]) -> Result<()> {
    let file_size = in_stream.seek(SeekFrom::End(0))?;
    in_stream.seek(SeekFrom::Start(0))?;

    let cipher = Aes128CbcDec::new(key.into(), iv.into());

    while in_stream.stream_position()? < file_size {
        let atom = match Atom::read(in_stream)? {
            Some(atom) => atom,
            None => break,
        };

        match atom.atom_type {
            FTYP_ATOM_TYPE => {
                let new_atom = Atom {
                    size: 32,
                    atom_type: FTYP_ATOM_TYPE,
                };
                new_atom.write(out_stream)?;
                for tag in FTYP_TAGS.iter() {
                    out_stream.write_all(&tag.to_be_bytes())?;
                }
                // seek past the original ftyp atom content
                in_stream.seek(SeekFrom::Current(atom.size as i64 - 8))?;
            }
            MDAT_ATOM_TYPE => {
                atom.write(out_stream)?;
                let mdat_end = in_stream.stream_position()? + atom.size - 8;
                while in_stream.stream_position()? < mdat_end {
                    let mut aavd = match AavdAtom::read(in_stream)? {
                        Some(aavd) => aavd,
                        None => break,
                    };

                    if aavd.atom_type == AAVD_ATOM_TYPE {
                        aavd.atom_type = MP4A_ATOM_TYPE;
                        let decrypted_data = cipher.clone().decrypt_padded_vec_mut::<cbc::cipher::block_padding::NoPadding>(&aavd.data).unwrap();
                        aavd.data = decrypted_data;
                    }
                    aavd.write(out_stream)?;
                }
            }
            _ => {
                atom.write(out_stream)?;
                let mut content = vec![0; (atom.size - 8) as usize];
                in_stream.read_exact(&mut content)?;
                out_stream.write_all(&content)?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}
