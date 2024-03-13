use std::borrow::Cow;
use std::{
    convert::TryFrom,
    io::{Cursor, Read, Seek, SeekFrom},
    marker::PhantomData,
};

use byteorder::{ByteOrder, ReadBytesExt};

use crate::io::read::{BdatReader, BdatSlice};
use crate::io::BDAT_MAGIC;
use crate::legacy::float::BdatReal;
use crate::{
    error::{BdatError, Result, Scope},
    BdatFile, ColumnDef, Label, ModernRow, ModernTable, ModernTableBuilder, Utf, Value, ValueType,
};

use super::FileHeader;

const LEN_COLUMN_DEF_V2: usize = 3;
const LEN_HASH_DEF_V2: usize = 8;

pub struct FileReader<R, E> {
    tables: TableReader<R, E>,
    header: FileHeader,
    _endianness: PhantomData<E>,
}

struct TableData<'r> {
    data: Cow<'r, [u8]>,
    string_table_offset: usize,
}

pub trait ModernRead<'b> {
    /// Read a single 32-bit unsigned integer at the current position.
    fn read_u32(&mut self) -> Result<u32>;

    /// Get a slice (or buffer) to the full binary stream for a single table.
    fn read_table_data(&mut self, length: usize) -> Result<Cow<'b, [u8]>>;

    /// Seek the current position to the next table at the given offset.
    fn seek_table(&mut self, offset: usize) -> Result<()>;
}

struct HeaderReader<R, E> {
    reader: R,
    _endianness: PhantomData<E>,
}

struct TableReader<R, E> {
    reader: R,
    _endianness: PhantomData<E>,
}

impl<'b, R, E> FileReader<R, E>
where
    R: ModernRead<'b>,
    E: ByteOrder,
{
    pub(crate) fn read_file(mut reader: R) -> Result<Self> {
        if reader.read_u32()? == u32::from_le_bytes(BDAT_MAGIC) {
            if reader.read_u32()? != 0x01_00_10_04 {
                return Err(BdatError::MalformedBdat(Scope::File));
            }
            Self::new_with_header(reader)
        } else {
            Err(BdatError::MalformedBdat(Scope::File))
        }
    }

    fn read_table(&mut self) -> Result<ModernTable<'b>> {
        self.tables.read_table_v2()
    }

    fn new_with_header(reader: R) -> Result<Self> {
        let mut header_reader = HeaderReader::<R, E>::new(reader);
        let header = header_reader.read_header()?;
        Ok(Self {
            tables: TableReader::new(header_reader.reader),
            header,
            _endianness: PhantomData,
        })
    }
}

impl<'b, R: ModernRead<'b>, E: ByteOrder> HeaderReader<R, E> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            _endianness: PhantomData,
        }
    }

    fn read_header(&mut self) -> Result<FileHeader> {
        let table_count = self.reader.read_u32()? as usize;
        let mut table_offsets = Vec::with_capacity(table_count);

        self.reader.read_u32()?; // File size

        for _ in 0..table_count {
            table_offsets.push(self.reader.read_u32()? as usize);
        }

        Ok(FileHeader {
            table_count,
            table_offsets,
        })
    }
}

impl<'b, R: ModernRead<'b>, E: ByteOrder> TableReader<R, E> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            _endianness: PhantomData,
        }
    }

    fn read_table_v2(&mut self) -> Result<ModernTable<'b>> {
        if self.reader.read_u32()? != u32::from_le_bytes(BDAT_MAGIC)
            || self.reader.read_u32()? != 0x3004
        {
            return Err(BdatError::MalformedBdat(Scope::Table));
        }

        let columns = self.reader.read_u32()? as usize;
        let rows = self.reader.read_u32()? as usize;
        let base_id = self.reader.read_u32()?;
        if self.reader.read_u32()? != 0 {
            panic!("Found unknown value at index 0x14 that was not 0");
        }

        let offset_col = self.reader.read_u32()? as usize;
        let offset_hash = self.reader.read_u32()? as usize;
        let offset_row = self.reader.read_u32()? as usize;
        #[allow(clippy::needless_late_init)]
        let offset_string;

        let row_length = self.reader.read_u32()? as usize;
        offset_string = self.reader.read_u32()? as usize;
        let str_length = self.reader.read_u32()? as usize;

        let lengths = [
            offset_col + LEN_COLUMN_DEF_V2 * columns,
            offset_hash + LEN_HASH_DEF_V2 * rows,
            offset_row + row_length * rows,
            offset_string + str_length,
        ];
        let table_len = lengths
            .iter()
            .max_by_key(|&i| i)
            .expect("could not determine table length");
        let table_raw = self.reader.read_table_data(*table_len)?;
        let table_data = TableData::new(table_raw, offset_string);

        let name = table_data.get_name::<E>()?;
        let mut col_data = Vec::with_capacity(columns);
        let mut row_data = Vec::with_capacity(rows);

        for i in 0..columns {
            let col = &table_data.data[offset_col + i * LEN_COLUMN_DEF_V2..];
            let ty =
                ValueType::try_from(col[0]).map_err(|_| BdatError::UnknownValueType(col[0]))?;
            let name_offset = (&col[1..]).read_u16::<E>()?;
            let label = table_data.get_label::<E>(name_offset as usize)?;

            col_data.push(ColumnDef {
                value_type: ty,
                label,
                flags: Vec::new(),
                count: 1,
            });
        }

        for i in 0..rows {
            let row = &table_data.data[offset_row + i * row_length..];
            let mut values = Vec::with_capacity(col_data.len());
            let mut cursor = Cursor::new(row);
            for col in &col_data {
                let value = Self::read_value(&table_data, &mut cursor, col.value_type)?;
                values.push(value);
            }
            row_data.push(ModernRow::new(values));
        }

        Ok(ModernTableBuilder::with_name(name)
            .set_base_id(base_id)
            .set_columns(col_data)
            .set_rows(row_data)
            .build())
    }

    fn read_value(
        table_data: &TableData<'b>,
        mut buf: impl Read,
        col_type: ValueType,
    ) -> Result<Value<'b>> {
        Ok(match col_type {
            ValueType::Unknown => Value::Unknown,
            ValueType::UnsignedByte => Value::UnsignedByte(buf.read_u8()?),
            ValueType::UnsignedShort => Value::UnsignedShort(buf.read_u16::<E>()?),
            ValueType::UnsignedInt => Value::UnsignedInt(buf.read_u32::<E>()?),
            ValueType::SignedByte => Value::SignedByte(buf.read_i8()?),
            ValueType::SignedShort => Value::SignedShort(buf.read_i16::<E>()?),
            ValueType::SignedInt => Value::SignedInt(buf.read_i32::<E>()?),
            ValueType::String => {
                Value::String(table_data.get_string(buf.read_u32::<E>()? as usize, usize::MAX)?)
            }
            ValueType::Float => Value::Float(BdatReal::Floating(buf.read_f32::<E>()?.into())),
            ValueType::Percent => Value::Percent(buf.read_u8()?),
            ValueType::HashRef => Value::HashRef(buf.read_u32::<E>()?),
            ValueType::DebugString => Value::DebugString(
                table_data.get_string(buf.read_u32::<E>()? as usize, usize::MAX)?,
            ),
            ValueType::Unknown2 => Value::Unknown2(buf.read_u8()?),
            ValueType::Unknown3 => Value::Unknown3(buf.read_u16::<E>()?),
        })
    }
}

impl<'r> TableData<'r> {
    fn new(data: Cow<'r, [u8]>, strings_offset: usize) -> TableData<'r> {
        Self {
            data,
            string_table_offset: strings_offset,
        }
    }

    /// Returns the table's hashed name, or [`None`] if it could not be found.
    fn get_name<E>(&self) -> Result<Label>
    where
        E: ByteOrder,
    {
        // First byte = 0 => labels are hashed. Otherwise, the string starts from the first byte
        let offset = if self.are_labels_hashed() { 1 } else { 0 };
        self.get_label::<E>(offset)
    }

    /// Reads a null-terminated UTF-8 encoded string from the string table at the given offset
    fn get_string(&self, offset: usize, limit: usize) -> Result<Utf<'r>> {
        let str_ptr = self.string_table_offset + offset;
        let len = self.data[str_ptr..]
            .split(|&b| b == 0)
            .take(1)
            .flatten()
            .take(limit)
            .count();
        let str = match &self.data {
            Cow::Borrowed(data) => {
                Cow::Borrowed(std::str::from_utf8(&data[str_ptr..str_ptr + len])?)
            }
            Cow::Owned(data) => {
                Cow::Owned(std::str::from_utf8(&data[str_ptr..str_ptr + len])?.to_string())
            }
        };
        Ok(str)
    }

    /// Reads a column label (either a string or a hash) from the string table at the given offset
    fn get_label<E>(&self, offset: usize) -> Result<Label>
    where
        E: ByteOrder,
    {
        if self.are_labels_hashed() {
            Ok(Label::Hash(
                (&self.data[self.string_table_offset + offset..]).read_u32::<E>()?,
            ))
        } else {
            Ok(Label::String(
                self.get_string(offset, usize::MAX)?.to_string(),
            ))
        }
    }

    fn are_labels_hashed(&self) -> bool {
        self.data[self.string_table_offset] == 0
    }
}

impl<'b, E> ModernRead<'b> for BdatSlice<'b, E>
where
    E: ByteOrder,
{
    fn read_table_data(&mut self, length: usize) -> Result<Cow<'b, [u8]>> {
        Ok(Cow::Borrowed(
            &self.data.clone().into_inner()[self.table_offset..self.table_offset + length],
        ))
    }

    #[inline]
    fn read_u32(&mut self) -> Result<u32> {
        Ok(self.data.read_u32::<E>()?)
    }

    fn seek_table(&mut self, offset: usize) -> Result<()> {
        self.data.seek(SeekFrom::Start(offset as u64))?;
        self.table_offset = offset;
        Ok(())
    }
}

impl<'b, R, E> ModernRead<'b> for BdatReader<R, E>
where
    R: Read + Seek,
    E: ByteOrder,
{
    fn read_table_data(&mut self, length: usize) -> Result<Cow<'b, [u8]>> {
        let mut table_raw = vec![0u8; length];
        self.stream
            .seek(SeekFrom::Start(self.table_offset as u64))?;
        self.stream.read_exact(&mut table_raw)?;
        Ok(table_raw.into())
    }

    #[inline]
    fn read_u32(&mut self) -> Result<u32> {
        Ok(self.stream.read_u32::<E>()?)
    }

    fn seek_table(&mut self, offset: usize) -> Result<()> {
        self.stream.seek(SeekFrom::Start(offset as u64))?;
        self.table_offset = offset;
        Ok(())
    }
}

impl<'b, R, E> BdatFile<'b> for FileReader<R, E>
where
    R: ModernRead<'b>,
    E: ByteOrder,
{
    type TableOut = ModernTable<'b>;

    /// Reads all tables from the BDAT source.
    fn get_tables(&mut self) -> Result<Vec<ModernTable<'b>>> {
        let mut tables = Vec::with_capacity(self.header.table_count);

        for i in 0..self.header.table_count {
            self.tables
                .reader
                .seek_table(self.header.table_offsets[i])?;
            let table = self.read_table()?;
            tables.push(table);
        }

        Ok(tables)
    }

    /// Returns the number of tables in the BDAT file.
    fn table_count(&self) -> usize {
        self.header.table_count
    }
}
