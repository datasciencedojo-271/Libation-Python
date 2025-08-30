use std::io::{Read, Write};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

pub struct Atom {
    pub size: u64,
    pub atom_type: u32,
}

impl Atom {
    pub fn read(reader: &mut impl Read) -> Result<Option<Self>, anyhow::Error> {
        let size = match reader.read_u32::<BigEndian>() {
            Ok(s) => s as u64,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let atom_type = reader.read_u32::<BigEndian>()?;

        let size = if size == 1 {
            reader.read_u64::<BigEndian>()?
        } else {
            size
        };

        Ok(Some(Self { size, atom_type }))
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), anyhow::Error> {
        if self.size > u32::MAX as u64 {
            writer.write_u32::<BigEndian>(1)?;
            writer.write_u32::<BigEndian>(self.atom_type)?;
            writer.write_u64::<BigEndian>(self.size)?;
        } else {
            writer.write_u32::<BigEndian>(self.size as u32)?;
            writer.write_u32::<BigEndian>(self.atom_type)?;
        }
        Ok(())
    }
}

pub struct AavdAtom {
    pub size: u32,
    pub atom_type: u32,
    pub data: Vec<u8>,
}

impl AavdAtom {
    pub fn read(reader: &mut impl Read) -> Result<Option<Self>, anyhow::Error> {
        let size = match reader.read_u32::<BigEndian>() {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let atom_type = reader.read_u32::<BigEndian>()?;

        let mut data = vec![0; (size - 8) as usize];
        reader.read_exact(&mut data)?;

        Ok(Some(Self { size, atom_type, data }))
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), anyhow::Error> {
        writer.write_u32::<BigEndian>(self.size)?;
        writer.write_u32::<BigEndian>(self.atom_type)?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}
