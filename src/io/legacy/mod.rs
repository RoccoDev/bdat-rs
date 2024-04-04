//! I/O operations for legacy BDATs

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
use std::num::NonZeroUsize;
use std::ops::Range;

use crate::error::Result;
use crate::legacy::read::{LegacyBytes, LegacyReader};
use crate::table::legacy::LegacyTable;
use crate::LegacyVersion;
use write::FileWriter;

pub(super) const HEADER_SIZE: usize = 64;
pub(super) const HEADER_SIZE_WII: usize = 32;
const COLUMN_NODE_SIZE: usize = 6;
const COLUMN_NODE_SIZE_WII: usize = 4;

pub use hash::HashTable as LegacyHashTable;

/// Additional options for writing legacy BDAT tables.
#[derive(Copy, Clone)]
pub struct LegacyWriteOptions {
    pub(crate) hash_slots: usize,
    pub(crate) scramble: bool,
    pub(crate) scramble_key: Option<u16>,
}

#[derive(Debug)]
pub struct FileHeader {
    pub table_count: usize,
    file_size: usize,
    table_offsets: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct TableHeader {
    pub scramble_type: ScrambleType,
    hashes: OffsetAndLen,
    strings: OffsetAndLen,
    offset_names: usize,
    offset_rows: usize,
    row_count: usize,
    row_len: usize,
    base_id: u16,
    columns: Option<ColumnNodeInfo>,
}

#[derive(Debug, Clone, Copy)]
struct ColumnNodeInfo {
    offset_columns: usize,
    column_count: usize,
}

#[derive(Debug, Clone, Copy)]
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
/// use bdat::{BdatFile, BdatResult, LegacyVersion, SwitchEndian};
///
/// fn read_file(name: &str) -> BdatResult<()> {
///     let file = File::open(name)?;
///     let tables = bdat::legacy::from_reader::<_, SwitchEndian>(file, LegacyVersion::Switch)?.get_tables();
///     Ok(())
/// }
/// ```
///
/// [`bdat::detect_file_version`]: crate::detect_file_version
/// [`bdat::from_reader`]: crate::from_reader
/// [`BdatFile::get_tables`]: crate::BdatFile::get_tables
pub fn from_reader<R: Read + Seek, E: ByteOrder>(
    reader: R,
    version: LegacyVersion,
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
/// use bdat::{BdatFile, BdatResult, LegacyVersion, SwitchEndian};
///
/// fn read(data: &mut [u8]) -> BdatResult<()> {
///     let tables = bdat::legacy::from_bytes::<SwitchEndian>(data, LegacyVersion::Switch)?.get_tables();
///     Ok(())
/// }
/// ```
///
/// [`bdat::detect_bytes_version`]: crate::detect_bytes_version
/// [`bdat::from_bytes`]: crate::from_bytes
/// [`BdatFile::get_tables`]: crate::BdatFile::get_tables
pub fn from_bytes<E: ByteOrder>(
    bytes: &mut [u8],
    version: LegacyVersion,
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
/// use bdat::{BdatFile, BdatResult, LegacyVersion, SwitchEndian};
///
/// fn read(data: &mut [u8]) -> BdatResult<()> {
///     let tables = bdat::legacy::from_bytes::<SwitchEndian>(data, LegacyVersion::Switch)?.get_tables();
///     Ok(())
/// }
/// ```
///
/// [`bdat::detect_bytes_version`]: crate::detect_bytes_version
/// [`bdat::from_bytes`]: crate::from_bytes
/// [`BdatFile::get_tables`]: crate::BdatFile::get_tables
pub fn from_bytes_copy<E: ByteOrder>(
    bytes: &[u8],
    version: LegacyVersion,
) -> Result<LegacyBytes<'_, E>> {
    LegacyBytes::new_copy(bytes, version)
}

/// Writes legacy BDAT tables to a [`std::io::Write`] implementation
/// that also implements [`std::io::Seek`].
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, SwitchEndian, LegacyVersion, legacy::LegacyTable};
///
/// fn write_file(name: &str, tables: &[LegacyTable]) -> BdatResult<()> {
///     let file = File::create(name)?;
///     // The legacy writer supports LegacyVersion:: and LegacyVersion::X
///     bdat::legacy::to_writer::<_, SwitchEndian>(file, tables, LegacyVersion::Switch)?;
///     Ok(())
/// }
/// ```
pub fn to_writer<'t, W: Write + Seek, E: ByteOrder + 'static>(
    writer: W,
    tables: impl IntoIterator<Item = impl Borrow<LegacyTable<'t>>>,
    version: LegacyVersion,
) -> Result<()> {
    to_writer_options::<W, E>(writer, tables, version, LegacyWriteOptions::new())
}

/// Writes legacy BDAT tables to a [`std::io::Write`] implementation
/// that also implements [`std::io::Seek`].
///
/// This function also allows customization of a few write options, using
/// [`LegacyWriteOptions`].
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, SwitchEndian, LegacyVersion};
/// use bdat::legacy::{LegacyWriteOptions, LegacyTable};
///
/// fn write_file(name: &str, tables: &[LegacyTable]) -> BdatResult<()> {
///     let file = File::create(name)?;
///     // The legacy writer supports LegacyVersion:: and LegacyVersion::X
///     bdat::legacy::to_writer_options::<_, SwitchEndian>(file, tables, LegacyVersion::Switch,
///             LegacyWriteOptions::new().hash_slots(10.try_into().unwrap()))?;
///     Ok(())
/// }
/// ```
pub fn to_writer_options<'t, W: Write + Seek, E: ByteOrder + 'static>(
    writer: W,
    tables: impl IntoIterator<Item = impl Borrow<LegacyTable<'t>>>,
    version: LegacyVersion,
    opts: LegacyWriteOptions,
) -> Result<()> {
    let mut writer = FileWriter::<W, E>::new(writer, version, opts);
    writer.write_file(tables)
}

/// Writes legacy BDAT tables to a `Vec<u8>`.
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, legacy::LegacyTable, SwitchEndian, LegacyVersion};
///
/// fn write_vec(tables: &[LegacyTable]) -> BdatResult<()> {
///     // The legacy writer supports LegacyVersion:: and LegacyVersion::X
///     let vec = bdat::legacy::to_vec::<SwitchEndian>(tables, LegacyVersion::Switch)?;
///     Ok(())
/// }
/// ```
pub fn to_vec<'t, E: ByteOrder + 'static>(
    tables: impl IntoIterator<Item = impl Borrow<LegacyTable<'t>>>,
    version: LegacyVersion,
) -> Result<Vec<u8>> {
    to_vec_options::<E>(tables, version, LegacyWriteOptions::new())
}

/// Writes legacy BDAT tables to a `Vec<u8>`.
///
/// This function also allows customization of a few write options, using
/// [`LegacyWriteOptions`].
///
/// ```
/// use std::fs::File;
/// use bdat::{BdatResult, SwitchEndian, LegacyVersion};
/// use bdat::legacy::{LegacyWriteOptions, LegacyTable};
///
/// fn write_vec(tables: &[LegacyTable]) -> BdatResult<()> {
///     // The legacy writer supports LegacyVersion:: and LegacyVersion::X
///     let vec = bdat::legacy::to_vec_options::<SwitchEndian>(tables, LegacyVersion::Switch,
///             LegacyWriteOptions::new().hash_slots(10.try_into().unwrap()))?;
///     Ok(())
/// }
/// ```
pub fn to_vec_options<'t, E: ByteOrder + 'static>(
    tables: impl IntoIterator<Item = impl Borrow<LegacyTable<'t>>>,
    version: LegacyVersion,
    opts: LegacyWriteOptions,
) -> Result<Vec<u8>> {
    let mut vec = Vec::new();
    to_writer_options::<_, E>(Cursor::new(&mut vec), tables, version, opts)?;
    Ok(vec)
}

impl LegacyWriteOptions {
    pub const fn new() -> Self {
        Self {
            hash_slots: 61, // used for all tables in 1/X/2/DE
            scramble: false,
            scramble_key: None, // calculated checksum by default
        }
    }

    /// Sets how big the generated hash table will be.
    ///
    /// A rule of thumb is that more slots translates to fewer collisions, however, due to the
    /// way the hashing algorithm works, some names might always hash to the same value, no
    /// matter the hash table size.
    ///
    /// The default value is 61.
    pub fn hash_slots(mut self, slots: NonZeroUsize) -> Self {
        self.hash_slots = slots.into();
        self
    }

    /// Sets whether tables should be scrambled during write.
    ///
    /// By default, tables are not scrambled.
    pub fn scramble(mut self, scramble: bool) -> Self {
        self.scramble = scramble;
        self
    }

    /// Sets the key used to scramble tables, if scrambling is enabled
    /// (see [`scramble`]).
    ///
    /// The default scramble key is calculated based on the table's checksum.
    pub fn scramble_key(mut self, scramble_key: u16) -> Self {
        self.scramble_key = Some(scramble_key);
        self
    }
}

impl Default for LegacyWriteOptions {
    fn default() -> Self {
        Self::new()
    }
}
