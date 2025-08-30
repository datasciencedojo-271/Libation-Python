use sha1::{Digest, Sha1};
use std::io::{Read, Seek, SeekFrom};
use cbc::cipher::{KeyIvInit, BlockDecryptMut};

const FIXED_KEY: [u8; 16] = [
    0x77, 0x21, 0x4d, 0x4b, 0x19, 0x6a, 0x87, 0xcd, 0x52, 0x00, 0x45, 0xfd, 0x20, 0xa5, 0x1d, 0x67,
];

const ADRM_START: u64 = 0x251;
const ADRM_LENGTH: usize = 56;
const CKSM_START: u64 = 0x28d;
const CKSM_LENGTH: usize = 20;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

#[derive(Debug, thiserror::Error)]
pub enum DerivationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Credential mismatch: either the activation bytes are incorrect or the audio file is invalid or corrupt.")]
    CredentialMismatch,
    #[error("Hex decoding error: {0}")]
    Hex(#[from] hex::FromHexError),
}

fn sha1_digest(data: &[&[u8]]) -> Vec<u8> {
    let mut hasher = Sha1::new();
    for d in data {
        hasher.update(d);
    }
    hasher.finalize().to_vec()
}

fn pad_16(data: &[u8]) -> Vec<u8> {
    let len = 16 - (data.len() % 16);
    let mut padded = data.to_vec();
    padded.resize(data.len() + len, len as u8);
    padded
}

fn swap_endian(hex_str: &str) -> String {
    hex_str
        .chars()
        .collect::<Vec<char>>()
        .chunks(2)
        .map(|c| c.iter().collect::<String>())
        .rev()
        .collect::<Vec<String>>()
        .join("")
}

pub fn derive_key_iv(
    mut in_stream: impl Read + Seek,
    activation_bytes: &str,
) -> Result<(Vec<u8>, Vec<u8>), DerivationError> {
    let file_start = in_stream.stream_position()?;
    let activation_bytes_decoded = &hex::decode(activation_bytes)?;

    let im_key = sha1_digest(&[&FIXED_KEY, activation_bytes_decoded]);
    let iv = sha1_digest(&[&FIXED_KEY, &im_key, activation_bytes_decoded]);
    let key = &im_key[..16];
    let iv_16 = &iv[..16];

    let cipher = Aes128CbcDec::new(key.into(), iv_16.into());

    in_stream.seek(SeekFrom::Start(ADRM_START))?;
    let mut adrm_data = vec![0u8; ADRM_LENGTH];
    in_stream.read_exact(&mut adrm_data)?;
    let padded_adrm = pad_16(&adrm_data);
    let decrypted_adrm = cipher.decrypt_padded_vec_mut::<cbc::cipher::block_padding::NoPadding>(&padded_adrm).unwrap(); // Should not fail with correct padding

    in_stream.seek(SeekFrom::Start(CKSM_START))?;
    let mut real_checksum = vec![0u8; CKSM_LENGTH];
    in_stream.read_exact(&mut real_checksum)?;

    let derived_checksum = sha1_digest(&[key, iv_16]);

    if derived_checksum[..] != real_checksum[..] {
        return Err(DerivationError::CredentialMismatch);
    }

    let real_bites = swap_endian(&hex::encode(&decrypted_adrm[..4]));
    if real_bites != *activation_bytes {
        return Err(DerivationError::CredentialMismatch);
    }

    let file_key = &decrypted_adrm[8..24];
    let file_drm = &decrypted_adrm[26..42];

    let in_vect = sha1_digest(&[file_drm, file_key, &FIXED_KEY]);

    in_stream.seek(SeekFrom::Start(file_start))?;

    Ok((file_key.to_vec(), in_vect[..16].to_vec()))
}
