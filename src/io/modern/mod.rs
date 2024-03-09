//! I/O operations for XC3 ("modern") BDATs

use std::borrow::Borrow;
use std::io::{Cursor, Read, Seek, Write};

use self::write::BdatWriter;
use super::read::{BdatReader, BdatSlice};
use crate::{error::Result, ModernTable};
use byteorder::ByteOrder;

// doc
#[allow(unused_imports)]
use crate::BdatFile;

mod read;
mod write;

pub use read::FileReader;

#[derive(Debug)]
pub(crate) struct FileHeader {
    pub table_count: usize,
    pub(crate) table_offsets: Vec<usize>,
}

/// Reads a BDAT file from a [`std::io::Read`] implementation. That type must also implement
/// [`std::io::Seek`].
///
/// This function will only read the file header. To parse tables, call [`BdatFile::get_tables`].
///
/// The BDAT file format is not recommended for streams, so it is best to read from a file or a
/// byte buffer.
///
/// Tables read using this function can be easily queried for single-value cells. See
/// [`ModernTable`] for details.
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, SwitchEndian};
///
/// fn read_file(name: &str) -> BdatResult<()> {
///     let file = File::open(name)?;
///     let file = bdat::modern::from_reader::<_, SwitchEndian>(file)?;
///     Ok(())
/// }
/// ```
pub fn from_reader<R: Read + Seek, E: ByteOrder>(
    reader: R,
) -> Result<FileReader<BdatReader<R, E>, E>> {
    FileReader::read_file(BdatReader::new(reader))
}

/// Reads a BDAT file from a slice. The slice needs to have the **full** file data, though any
/// unrelated bytes at the end will be ignored.
///
/// This function will only read the file header. To parse tables, call [`BdatFile::get_tables`].
///
/// Tables read using this function can be easily queried for single-value cells. See
/// [`ModernTable`] for details.
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, SwitchEndian};
///
/// fn read(data: &[u8]) -> BdatResult<()> {
///     let file = bdat::modern::from_bytes::<SwitchEndian>(data)?;
///     Ok(())
/// }
/// ```
pub fn from_bytes<E: ByteOrder>(bytes: &[u8]) -> Result<FileReader<BdatSlice<'_, E>, E>> {
    FileReader::read_file(BdatSlice::new(bytes))
}

/// Writes BDAT tables to a [`std::io::Write`] implementation that also implements [`std::io::Seek`].
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, SwitchEndian, ModernTable};
///
/// fn write_file(name: &str, tables: &[ModernTable]) -> BdatResult<()> {
///     let file = File::create(name)?;
///     bdat::modern::to_writer::<_, SwitchEndian>(file, tables)?;
///     Ok(())
/// }
/// ```
pub fn to_writer<'t, W: Write + Seek, E: ByteOrder>(
    writer: W,
    tables: impl IntoIterator<Item = impl Borrow<ModernTable<'t>>>,
) -> Result<()> {
    let mut writer = BdatWriter::<W, E>::new(writer);
    writer.write_file(tables)
}

/// Writes BDAT tables to a `Vec<u8>`.
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, SwitchEndian, ModernTable};
///
/// fn write_vec(tables: &[ModernTable]) -> BdatResult<()> {
///     let vec = bdat::modern::to_vec::<SwitchEndian>(tables)?;
///     Ok(())
/// }
/// ```
pub fn to_vec<'t, E: ByteOrder>(
    tables: impl IntoIterator<Item = impl Borrow<ModernTable<'t>>>,
) -> Result<Vec<u8>> {
    let mut vec = Vec::new();
    to_writer::<_, E>(Cursor::new(&mut vec), tables)?;
    Ok(vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::SwitchEndian, BdatFile, ColumnDef, Label, ModernRow, ModernTableBuilder, Value, ValueType,
    };

    #[test]
    fn table_write_back() {
        let table = ModernTableBuilder::with_name(Label::Hash(0xca_fe_ba_be))
            .add_column(ColumnDef::new(
                ValueType::HashRef,
                Label::Hash(0xde_ad_be_ef),
            ))
            .add_column(ColumnDef {
                value_type: ValueType::UnsignedInt,
                label: Label::Hash(0xca_fe_ca_fe),
                flags: Vec::new(),
                count: 1,
            })
            .add_row(ModernRow::new(
                vec![
                    Value::HashRef(0x00_00_00_01),
                    Value::UnsignedInt(10),
                ],
            ))
            .add_row(ModernRow::new(
                vec![
                    Value::HashRef(0x01_00_00_01),
                    Value::UnsignedInt(100),
                ],
            ))
            .build();

        let written = to_vec::<SwitchEndian>([&table]).unwrap();
        let read_back = &from_bytes::<SwitchEndian>(&written)
            .unwrap()
            .get_tables()
            .unwrap()[0];
        assert_eq!(table, *read_back);

        let new_written = to_vec::<SwitchEndian>([read_back]).unwrap();
        assert_eq!(written, new_written);
    }
}
