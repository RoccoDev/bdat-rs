use std::{fmt::Display, marker::PhantomData, ops::Index};

use enum_kinds::EnumKind;
use num_enum::TryFromPrimitive;

/// A memory-mapped Bdat table
pub struct MappedTable<'b, I, R> {
    buffer: &'b I,
    _ty: PhantomData<R>,
}

/// A Bdat table
///
/// ## Accessing cells
/// The [`RowRef`] struct provides an easy interface to access cells.  
/// For example, to access the cell at row 1 and column "Param1", you can use `table.row(1)["Param1".into()]`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawTable {
    pub name: Option<Label>,
    pub(crate) columns: Vec<ColumnDef>,
    pub(crate) rows: Vec<Row>,
}

/// A builder interface for [`RawTable`].
pub struct TableBuilder(RawTable);

/// A column definition from a Bdat table
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDef {
    pub ty: ValueType,
    pub label: Label,
    pub offset: usize,
}

/// A row from a Bdat table
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: usize,
    pub(crate) cells: Vec<Cell>,
}

/// A cell from a Bdat row
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize), serde(untagged))]
pub enum Cell {
    Single(Value),
    List(Vec<Value>),
    Flag(bool),
}

/// A value in a Bdat cell
#[derive(EnumKind, Debug, Clone, PartialEq)]
#[enum_kind(
    ValueType,
    derive(TryFromPrimitive),
    repr(u8),
    cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize)),
    cfg_attr(feature = "serde", serde(into = "u8", try_from = "u8"))
)]
pub enum Value {
    Unknown,
    UnsignedByte(u8),
    UnsignedShort(u16),
    UnsignedInt(u32),
    SignedByte(i8),
    SignedShort(i16),
    SignedInt(i32),
    String(String),
    Float(f32),
    HashRef(u32),
    Percent(u8),
    Unknown1(u32),
    Unknown2(u8),
    Unknown3(u16),
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Label {
    Hash(u32),
    String(String),
    Unhashed(String),
}

pub struct RowRef<'t> {
    index: usize,
    table: &'t RawTable,
}

impl Label {
    /// Extracts a [`Label`] from a [`String`].
    ///
    /// The format is as follows:  
    /// * `<01ABCDEF>` (8 hex digits) => `Label::Hash(0x01abcdef)`
    /// * s => `Label::String(s)`
    ///
    /// If `force_hash` is `true`, the label will be re-hashed
    /// if it is either [`Label::String`] or [`Label::Unhashed`].
    pub fn parse(text: String, force_hash: bool) -> Self {
        if text.len() == 10 && text.as_bytes()[0] == b'<' {
            if let Ok(n) = u32::from_str_radix(&text[1..=8], 16) {
                return Label::Hash(n);
            }
        }
        if force_hash {
            Label::Hash(crate::hash::murmur3_str(&text))
        } else {
            Label::String(text)
        }
    }
}

impl RawTable {
    pub fn new(name: Option<Label>, columns: Vec<ColumnDef>, rows: Vec<Row>) -> Self {
        Self {
            name,
            columns,
            rows,
        }
    }

    /// Returns the table's name, or [`None`] if the table has no
    /// name associated to it.
    pub fn name(&self) -> Option<&Label> {
        self.name.as_ref()
    }

    /// Updates the table's name.
    pub fn set_name(&mut self, name: Option<Label>) {
        self.name = name;
    }

    /// Gets a row by its ID
    ///
    /// # Panics
    /// If there is no row for the given ID
    pub fn row(&self, id: usize) -> RowRef<'_> {
        self.get_row(id).expect("no such row")
    }

    /// Attempts to get a row by its ID.  
    /// If there is no row for the given ID, this returns [`None`].
    pub fn get_row(&self, id: usize) -> Option<RowRef<'_>> {
        self.rows.get(id).map(|_| RowRef {
            index: id,
            table: self,
        })
    }

    /// Gets an iterator that visits this table's rows
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter()
    }

    /// Gets an iterator over mutable references to this table's
    /// rows.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.rows.iter_mut()
    }

    /// Gets an owning iterator over this table's rows
    pub fn into_rows(self) -> impl Iterator<Item = Row> {
        self.rows.into_iter()
    }

    /// Gets an iterator that visits this table's column definitions
    pub fn columns(&self) -> impl Iterator<Item = &ColumnDef> {
        self.columns.iter()
    }

    /// Gets an iterator over mutable references to this table's
    /// column definitions.
    pub fn columns_mut(&mut self) -> impl Iterator<Item = &mut ColumnDef> {
        self.columns.iter_mut()
    }

    /// Gets an owning iterator over this table's column definitions
    pub fn into_columns(self) -> impl Iterator<Item = ColumnDef> {
        self.columns.into_iter()
    }

    /// Gets the number of rows in the table
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Gets the number of columns in the table
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}

impl TableBuilder {
    pub fn new() -> Self {
        Self(RawTable::default())
    }

    pub fn set_name(&mut self, name: impl Into<Option<Label>>) -> &mut Self {
        self.0.set_name(name.into());
        self
    }

    pub fn add_column(&mut self, column: ColumnDef) -> &mut Self {
        self.0.columns.push(column);
        self
    }

    pub fn add_row(&mut self, row: Row) -> &mut Self {
        self.0.rows.push(row);
        self
    }

    pub fn set_rows(&mut self, rows: Vec<Row>) -> &mut Self {
        self.0.rows = rows;
        self
    }

    pub fn set_columns(&mut self, columns: Vec<ColumnDef>) -> &mut Self {
        self.0.columns = columns;
        self
    }

    pub fn build(&mut self) -> RawTable {
        std::mem::take(&mut self.0)
    }
}

impl Row {
    /// Creates a new [`Row`].
    pub fn new(id: usize, cells: Vec<Cell>) -> Self {
        Self { id, cells }
    }

    /// Gets an owning iterator over this row's cells
    pub fn into_cells(self) -> impl Iterator<Item = Cell> {
        self.cells.into_iter()
    }

    /// Gets an iterator over this row's cells
    pub fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.cells.iter()
    }
}

impl ValueType {
    pub fn data_len(&self) -> usize {
        use ValueType::*;
        match self {
            Unknown => 0,
            UnsignedByte | SignedByte | Percent | Unknown2 => 1,
            UnsignedShort | SignedShort | Unknown3 => 2,
            UnsignedInt | SignedInt | String | Float | HashRef | Unknown1 => 4,
        }
    }
}

impl<'t, S> Index<S> for RowRef<'t>
where
    S: Into<Label>,
{
    type Output = Cell;

    fn index(&self, index: S) -> &Self::Output {
        let index = index.into();
        let index = self
            .table
            .columns
            .iter()
            .position(|col| col.label == index)
            .expect("no such column");
        &self.table.rows[self.index].cells[index]
    }
}

impl From<String> for Label {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<u32> for Label {
    fn from(hash: u32) -> Self {
        Self::Hash(hash)
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hash(hash) => {
                if f.sign_plus() {
                    write!(f, "{:08X}", hash)
                } else {
                    write!(f, "<{:08X}>", hash)
                }
            }
            Self::String(s) | Self::Unhashed(s) => write!(f, "{}", s),
        }
    }
}

impl From<ValueType> for u8 {
    fn from(t: ValueType) -> Self {
        t as u8
    }
}

macro_rules! default_display {
    ($fmt:expr, $val:expr, $($variants:tt ) *) => {
        match $val {
            $(
                Value::$variants(a) => a.fmt($fmt),
            )*
            v => panic!("Unsupported Display {:?}", v)
        }
    };
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => return Ok(()),
            Self::HashRef(h) => Label::Hash(*h).fmt(f),
            Self::Percent(v) => write!(f, "{}%", v),
            v => {
                default_display!(f, v, SignedByte SignedShort SignedInt UnsignedByte UnsignedShort UnsignedInt Unknown1 Unknown2 Unknown3 String Float)
            }
        }
    }
}
