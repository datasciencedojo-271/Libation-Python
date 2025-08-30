use std::io::{Read, Seek, Write, SeekFrom};
use anyhow::Result;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};

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

    pub fn decrypt_to_file(&self, mut in_stream: impl Read + Seek, mut out_stream: impl Write) -> Result<()> {
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

        process_file(&mut in_stream, &mut out_stream, &key, &iv)?;

        self.step_get_metadata()?;

        Ok(())
    }

    fn step_get_metadata(&self) -> Result<()> {
        // TODO: Parse metadata from the decrypted file and apply modifications.
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


use cbc::cipher::BlockEncryptMut;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

    struct MockDownloadOptions {
        keys: Vec<KeyData>,
        file_type: FileType,
    }

    impl DownloadOptions for MockDownloadOptions {
        fn decryption_keys(&self) -> Option<&[KeyData]> {
            Some(&self.keys)
        }
        fn input_type(&self) -> FileType {
            self.file_type
        }
        fn strip_unabridged(&self) -> bool { false }
        fn fixup_file(&self) -> bool { false }
        fn title(&self) -> &str { "title" }
        fn subtitle(&self) -> Option<&str> { None }
        fn publisher(&self) -> Option<&str> { None }
        fn language(&self) -> Option<&str> { None }
        fn audible_product_id(&self) -> Option<&str> { None }
        fn series_name(&self) -> Option<&str> { None }
        fn series_number(&self) -> Option<&str> { None }
    }


    #[test]
    fn test_process_file_aaxc() {
        let key = hex::decode("0102030405060708090a0b0c0d0e0f10").unwrap();
        let iv = hex::decode("100f0e0d0c0b0a090807060504030201").unwrap();

        let opts = MockDownloadOptions {
            keys: vec![KeyData {
                key_part1: key.clone(),
                key_part2: Some(iv.clone()),
            }],
            file_type: FileType::Aaxc,
        };
        let converter = AaxcDownloadConvertBase::new("/tmp", "/tmp", opts);

        let mut input_data = Vec::new();
        // ftyp atom
        input_data.extend_from_slice(&32u32.to_be_bytes());
        input_data.extend_from_slice(&FTYP_ATOM_TYPE.to_be_bytes());
        input_data.extend_from_slice(&[0; 24]);
        // mdat atom
        let plain_text = "This is a test of the decryption logic.";
        let mut cipher = Aes128CbcEnc::new_from_slices(&key, &iv).unwrap();
        let encrypted_data = cipher.encrypt_padded_vec_mut::<cbc::cipher::block_padding::Pkcs7>(plain_text.as_bytes());
        let mdat_size = 8 + 8 + encrypted_data.len() as u32;
        input_data.extend_from_slice(&mdat_size.to_be_bytes());
        input_data.extend_from_slice(&MDAT_ATOM_TYPE.to_be_bytes());
        // aavd atom
        input_data.extend_from_slice(&(8u32 + encrypted_data.len() as u32).to_be_bytes());
        input_data.extend_from_slice(&AAVD_ATOM_TYPE.to_be_bytes());
        input_data.extend_from_slice(&encrypted_data);

        let mut in_stream = Cursor::new(input_data);
        let mut out_stream = Cursor::new(Vec::new());

        converter.decrypt_to_file(&mut in_stream, &mut out_stream).unwrap();

        let output_data = out_stream.into_inner();

        // Very basic verification
        assert!(output_data.len() > 0);
        let decrypted_text_with_padding = &output_data[48..];
        let unpadded_len = decrypted_text_with_padding.len() - *decrypted_text_with_padding.last().unwrap() as usize;
        let decrypted_text = &decrypted_text_with_padding[..unpadded_len];
        let decrypted_text_str = std::str::from_utf8(decrypted_text).unwrap();
        assert_eq!(decrypted_text_str, plain_text);
    }
}
