use crate::error::Result;
use crate::{Label, TableAccessor};
use std::collections::HashMap;
use std::io::Cursor;
use std::marker::PhantomData;

pub struct BdatReader<R, E> {
    pub(crate) stream: R,
    pub(crate) table_offset: usize,
    _endianness: PhantomData<E>,
}

#[derive(Clone)]
pub struct BdatSlice<'b, E> {
    pub(crate) data: Cursor<&'b [u8]>,
    pub(crate) table_offset: usize,
    _endianness: PhantomData<E>,
}

pub trait BdatFile<'b> {
    /// The output table type
    type TableOut;

    /// Reads all tables from the BDAT source.
    ///
    /// ## Future compatibility
    /// This function might start returning an iterator when Rust 1.75.0
    /// hits stable (specifically [this issue](https://github.com/rust-lang/rust/issues/91611)).
    fn get_tables(&mut self) -> Result<Vec<Self::TableOut>>;

    /// Returns the number of tables in the BDAT file.
    fn table_count(&self) -> usize;

    /// Reads all tables from the BDAT source, then groups them by name.
    ///
    /// ## Future compatibility
    /// This function might start returning an iterator when Rust 1.75.0
    /// hits stable (specifically [this issue](https://github.com/rust-lang/rust/issues/91611)).
    fn get_tables_by_name<'t, 'tb: 't>(&mut self) -> Result<HashMap<Label, Self::TableOut>>
    where
        Self::TableOut: TableAccessor<'t, 'tb>,
    {
        self.get_tables()
            .map(|tables| tables.into_iter().map(|t| (t.name().clone(), t)).collect())
    }
}

impl<'b, E> BdatSlice<'b, E> {
    pub fn new(bytes: &'b [u8]) -> Self {
        Self {
            data: Cursor::new(bytes),
            table_offset: 0,
            _endianness: PhantomData,
        }
    }
}

impl<R, E> BdatReader<R, E> {
    pub fn new(reader: R) -> Self {
        Self {
            stream: reader,
            table_offset: 0,
            _endianness: PhantomData,
        }
    }
}
