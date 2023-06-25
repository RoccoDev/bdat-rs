pub mod float;
pub mod scramble;

mod hash;
pub(crate) mod read;
mod util;
mod write;

use byteorder::ByteOrder;
use scramble::ScrambleType;
use std::borrow::Borrow;
use std::io::{Cursor, Read, Seek, Write};
use std::ops::Range;

use crate::error::Result;
use crate::legacy::read::{LegacyBytes, LegacyReader};
use crate::legacy::write::FileWriter;
use crate::{BdatVersion, Table};

const HEADER_SIZE: usize = 64;
const COLUMN_DEFINITION_SIZE: usize = 6;

pub use hash::HashTable as LegacyHashTable;

#[derive(Debug)]
pub struct FileHeader {
    pub table_count: usize,
    file_size: usize,
    table_offsets: Vec<usize>,
}

#[derive(Debug)]
pub struct TableHeader {
    pub scramble_type: ScrambleType,
    hashes: OffsetAndLen,
    strings: OffsetAndLen,
    offset_names: usize,
    offset_columns: usize,
    offset_rows: usize,
    column_count: usize,
    row_count: usize,
    row_len: usize,
    base_id: usize,
}

#[derive(Debug)]
struct OffsetAndLen {
    offset: usize,
    len: usize,
}

impl OffsetAndLen {
    fn max_offset(&self) -> usize {
        self.offset + self.len
    }

    fn range(&self) -> Range<usize> {
        self.offset..self.offset + self.len
    }
}

impl From<(usize, usize)> for OffsetAndLen {
    fn from((offset, len): (usize, usize)) -> Self {
        Self { offset, len }
    }
}

/// Reads a legacy BDAT file from a [`std::io::Read`] implementation. That type must also implement
/// [`std::io::Seek`].
///
/// With legacy files, the format version must be known in advance. To automatically detect
/// it from the file, use [`bdat::detect_file_version`], or read the file using
/// [`bdat::from_reader`].
///
/// This function will only read the file header. To parse tables, call [`BdatFile::get_tables`].
///
/// The BDAT file format is not recommended for streams, so it is best to read from a file or a
/// byte buffer.
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatFile, BdatResult, BdatVersion, SwitchEndian};
///
/// fn read_file(name: &str) -> BdatResult<()> {
///     let file = File::open(name)?;
///     let tables = bdat::legacy::from_reader::<_, SwitchEndian>(file, BdatVersion::Legacy)?.get_tables();
///     Ok(())
/// }
/// ```
pub fn from_reader<R: Read + Seek, E: ByteOrder>(
    reader: R,
    version: BdatVersion,
) -> Result<LegacyReader<R, E>> {
    LegacyReader::new(reader, version)
}

/// Reads a BDAT file from a slice. The slice needs to have the **full** file data, though any
/// unrelated bytes at the end will be ignored.
///
/// With legacy files, the format version must be known in advance. To automatically detect
/// it from the file, use [`bdat::detect_bytes_version`], or read the file using
/// [`bdat::from_bytes`].
///
/// Additionally, this function needs a mutable reference to the underlying data, as it may need
/// to unscramble text to properly read the file. To work around this restriction (by allowing
/// copies), [`from_bytes_copy`] can be used instead.
///
/// This function will only read the file header. To parse tables, call [`BdatFile::get_tables`].
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatFile, BdatResult, BdatVersion, SwitchEndian};
///
/// fn read(data: &mut [u8]) -> BdatResult<()> {
///     let tables = bdat::legacy::from_bytes::<SwitchEndian>(data, BdatVersion::Legacy)?.get_tables();
///     Ok(())
/// }
/// ```
pub fn from_bytes<E: ByteOrder>(
    bytes: &mut [u8],
    version: BdatVersion,
) -> Result<LegacyBytes<'_, E>> {
    LegacyBytes::new(bytes, version)
}

/// Reads a BDAT file from a slice. The slice needs to have the **full** file data, though any
/// unrelated bytes at the end will be ignored.
///
/// With legacy files, the format version must be known in advance. To automatically detect
/// it from the file, use [`bdat::detect_bytes_version`], or read the file using
/// [`bdat::from_bytes`].
///
/// Unlike [`from_bytes`], this doesn't require mutable access to the data, at the cost of
/// potentially copying the data if there's a need to unscramble it.
///
/// This function will only read the file header. To parse tables, call [`BdatFile::get_tables`].
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatFile, BdatResult, BdatVersion, SwitchEndian};
///
/// fn read(data: &mut [u8]) -> BdatResult<()> {
///     let tables = bdat::legacy::from_bytes::<SwitchEndian>(data, BdatVersion::Legacy)?.get_tables();
///     Ok(())
/// }
/// ```
pub fn from_bytes_copy<E: ByteOrder>(
    bytes: &[u8],
    version: BdatVersion,
) -> Result<LegacyBytes<'_, E>> {
    LegacyBytes::new_copy(bytes, version)
}

/// Writes legacy BDAT tables to a [`std::io::Write`] implementation
/// that also implements [`std::io::Seek`].
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, Table, SwitchEndian, BdatVersion};
///
/// fn write_file(name: &str, tables: &[Table]) -> BdatResult<()> {
///     let file = File::create(name)?;
///     // The legacy writer supports BdatVersion::Legacy and BdatVersion::LegacyX
///     bdat::legacy::to_writer::<_, SwitchEndian>(file, tables, BdatVersion::Legacy)?;
///     Ok(())
/// }
/// ```
pub fn to_writer<'t, W: Write + Seek, E: ByteOrder>(
    writer: W,
    tables: impl IntoIterator<Item = impl Borrow<Table<'t>>>,
    version: BdatVersion,
) -> Result<()> {
    let mut writer = FileWriter::<W, E>::new(writer, version);
    writer.write_file(tables)
}

/// Writes legacy BDAT tables to a `Vec<u8>`.
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, Table, SwitchEndian, BdatVersion};
///
/// fn write_vec(tables: &[Table]) -> BdatResult<()> {
///     // The legacy writer supports BdatVersion::Legacy and BdatVersion::LegacyX
///     let vec = bdat::legacy::to_vec::<SwitchEndian>(tables, BdatVersion::Legacy)?;
///     Ok(())
/// }
/// ```
pub fn to_vec<'t, E: ByteOrder>(
    tables: impl IntoIterator<Item = impl Borrow<Table<'t>>>,
    version: BdatVersion,
) -> Result<Vec<u8>> {
    let mut vec = Vec::new();
    to_writer::<_, E>(Cursor::new(&mut vec), tables, version)?;
    Ok(vec)
}
