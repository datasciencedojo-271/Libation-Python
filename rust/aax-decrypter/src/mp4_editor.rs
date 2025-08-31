use anyhow::Result;
use std::io::{Read, Seek, Write, SeekFrom};
use crate::atom::Atom;

const MOOV_ATOM_TYPE: u32 = 0x6d6f6f76;
const STCO_ATOM_TYPE: u32 = 0x7374636f;
const CO64_ATOM_TYPE: u32 = 0x636f3634;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

fn update_stco_atom(moov_buffer: &mut [u8], moov_size: u64) -> Result<()> {
    let mut cursor = std::io::Cursor::new(moov_buffer);
    while let Ok(Some(atom)) = Atom::read(&mut cursor) {
        let atom_start = cursor.position() - 8;
        if atom.atom_type == STCO_ATOM_TYPE {
            cursor.seek(SeekFrom::Current(4))?; // Skip version and flags
            let entry_count = cursor.read_u32::<BigEndian>()?;
            for _ in 0..entry_count {
                let offset_pos = cursor.position();
                let offset = cursor.read_u32::<BigEndian>()?;
                cursor.seek(SeekFrom::Start(offset_pos))?;
                cursor.write_u32::<BigEndian>(offset + moov_size as u32)?;
            }
            break;
        } else if atom.atom_type == CO64_ATOM_TYPE {
            cursor.seek(SeekFrom::Current(4))?; // Skip version and flags
            let entry_count = cursor.read_u32::<BigEndian>()?;
            for _ in 0..entry_count {
                let offset_pos = cursor.position();
                let offset = cursor.read_u64::<BigEndian>()?;
                cursor.seek(SeekFrom::Start(offset_pos))?;
                cursor.write_u64::<BigEndian>(offset + moov_size)?;
            }
            break;
        }
        cursor.seek(SeekFrom::Start(atom_start + atom.size as u64))?;
    }
    Ok(())
}

fn find_moov_atom(in_stream: &mut (impl Read + Seek)) -> Result<Option<(u64, Atom)>> {
    let file_size = in_stream.seek(SeekFrom::End(0))?;
    in_stream.seek(SeekFrom::Start(0))?;

    while in_stream.stream_position()? < file_size {
        let atom_offset = in_stream.stream_position()?;
        let atom = match Atom::read(in_stream)? {
            Some(atom) => atom,
            None => break,
        };

        if atom.atom_type == MOOV_ATOM_TYPE {
            return Ok(Some((atom_offset, atom)));
        }

        in_stream.seek(SeekFrom::Current(atom.size as i64 - 8))?;
    }

    Ok(None)
}

const FTYP_ATOM_TYPE: u32 = 0x66747970;
const MDAT_ATOM_TYPE: u32 = 0x6d646174;

pub fn move_moov_to_beginning(in_stream: &mut (impl Read + Seek + Write)) -> Result<()> {
    let mut temp_file = tempfile::tempfile()?;

    // 1. Find and write ftyp atom
    in_stream.seek(SeekFrom::Start(0))?;
    let ftyp_atom = Atom::read(in_stream)?.ok_or_else(|| anyhow::anyhow!("ftyp atom not found"))?;
    if ftyp_atom.atom_type != FTYP_ATOM_TYPE {
        return Err(anyhow::anyhow!("First atom is not ftyp"));
    }
    ftyp_atom.write(&mut temp_file)?;
    let mut ftyp_content = vec![0; (ftyp_atom.size - 8) as usize];
    in_stream.read_exact(&mut ftyp_content)?;
    temp_file.write_all(&ftyp_content)?;

    // 2. Find, modify, and write moov atom
    if let Some((moov_offset, moov_atom)) = find_moov_atom(in_stream)? {
        in_stream.seek(SeekFrom::Start(moov_offset + 8))?;
        let mut moov_buffer = vec![0; (moov_atom.size - 8) as usize];
        in_stream.read_exact(&mut moov_buffer)?;
        update_stco_atom(&mut moov_buffer, moov_atom.size)?;
        moov_atom.write(&mut temp_file)?;
        temp_file.write_all(&moov_buffer)?;
    } else {
        return Err(anyhow::anyhow!("moov atom not found"));
    }

    // 3. Find and copy mdat atom
    in_stream.seek(SeekFrom::Start(0))?;
    let file_size = in_stream.seek(SeekFrom::End(0))?;
    in_stream.seek(SeekFrom::Start(0))?;
    while in_stream.stream_position()? < file_size {
        let atom = Atom::read(in_stream)?.ok_or_else(|| anyhow::anyhow!("Error reading atom"))?;
        if atom.atom_type == MDAT_ATOM_TYPE {
            atom.write(&mut temp_file)?;
            let mut content = in_stream.take(atom.size - 8);
            std::io::copy(&mut content, &mut temp_file)?;
            break;
        }
        in_stream.seek(SeekFrom::Current(atom.size as i64 - 8))?;
    }

    // 4. Replace original file
    in_stream.seek(SeekFrom::Start(0))?;
    temp_file.seek(SeekFrom::Start(0))?;
    std::io::copy(&mut temp_file, in_stream)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_move_moov_to_beginning() {
        let mut file_data = Vec::new();
        // ftyp
        let ftyp_atom = Atom {
            size: 32,
            atom_type: FTYP_ATOM_TYPE,
        };
        ftyp_atom.write(&mut file_data).unwrap();
        file_data.extend_from_slice(&[0; 24]);
        // mdat
        let mdat_atom = Atom {
            size: 16,
            atom_type: MDAT_ATOM_TYPE,
        };
        mdat_atom.write(&mut file_data).unwrap();
        file_data.extend_from_slice(&[0; 8]);
        // moov
        let mut stco_content = Vec::new();
        stco_content.extend_from_slice(&[0; 4]); // version and flags
        stco_content.extend_from_slice(&1u32.to_be_bytes()); // entry count
        stco_content.extend_from_slice(&32u32.to_be_bytes()); // offset
        let stco_atom = Atom {
            size: (stco_content.len() + 8) as u64,
            atom_type: STCO_ATOM_TYPE,
        };
        let mut moov_content = Vec::new();
        stco_atom.write(&mut moov_content).unwrap();
        moov_content.extend_from_slice(&stco_content);

        let moov_atom = Atom {
            size: (moov_content.len() + 8) as u64,
            atom_type: MOOV_ATOM_TYPE,
        };
        moov_atom.write(&mut file_data).unwrap();
        file_data.extend_from_slice(&moov_content);

        let mut file = Cursor::new(file_data);
        move_moov_to_beginning(&mut file).unwrap();

        let mut new_file_data = Vec::new();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.read_to_end(&mut new_file_data).unwrap();

        // ftyp is 32 bytes, moov is 28 bytes. mdat starts at 60
        // stco offset should be 32 + 28 = 60
        let new_offset = &new_file_data[56..60];
        assert_eq!(u32::from_be_bytes(new_offset.try_into().unwrap()), 60);
    }

    #[test]
    fn test_update_stco_atom() {
        let mut moov_buffer = Vec::new();
        // stco
        let mut stco_content = Vec::new();
        stco_content.extend_from_slice(&[0; 4]); // version and flags
        stco_content.extend_from_slice(&1u32.to_be_bytes()); // entry count
        stco_content.extend_from_slice(&32u32.to_be_bytes()); // offset
        let stco_atom = Atom {
            size: (stco_content.len() + 8) as u64,
            atom_type: STCO_ATOM_TYPE,
        };
        stco_atom.write(&mut moov_buffer).unwrap();
        moov_buffer.extend_from_slice(&stco_content);

        update_stco_atom(&mut moov_buffer, 40).unwrap();

        let new_offset = &moov_buffer[16..20];
        assert_eq!(u32::from_be_bytes(new_offset.try_into().unwrap()), 72);
    }
}
