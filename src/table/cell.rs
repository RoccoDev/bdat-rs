use crate::legacy::float::BdatReal;
use crate::{BdatError, BdatResult, BdatVersion, Label};
use enum_kinds::EnumKind;
use num_enum::TryFromPrimitive;
use std::borrow::Cow;
use std::fmt::Display;

/// A cell from a BDAT row.
///
/// ## Cell types
/// There are three types of cells in the various iterations of the BDAT format:
/// * Single-value cells ([`Cell::Single`]), containing a single [`Value`].
/// * Arrays ([`Cell::List`]), containing multiple [`Value`]s, but all of the same type.
/// * Flag containers ([`Cell::Flags`]), stored as a number, but interpreted as flags by masking
/// bits.
///
/// Modern BDAT versions only support single-value cells.
///
/// ## Serialization
/// When the `serde` feature flag is enabled, cells can be serialized and deserialized using
/// Serde.
///
/// Cells don't store metadata about their type or the type of the values they contain.
/// Instead, they rely on columns to carry that data for them.
///
/// To serialize a `Cell` given its parent column, you can use [`Column::cell_serializer`].
/// ```
/// use bdat::{Cell, Column};
///
/// fn serialize_cell(column: &Column, cell: &Cell) -> String {
///     serde_json::to_string(&column.cell_serializer(cell)).unwrap()
/// }
/// ```
///
/// To deserialize a `Cell` that was serialized into the previous format, you can use
/// [`Column::as_cell_seed`], along with `DeserializeSeed` from Serde.
/// ```
/// use bdat::{Cell, Column};
/// use serde_json::Deserializer;
/// use serde::de::DeserializeSeed;
///
/// fn deserialize_cell<'s>(column: &Column, json: &'s str) -> Cell<'s> {
///     column.as_cell_seed().deserialize(&mut Deserializer::from_str(json)).unwrap()
/// }
/// ```
///
/// [`Column::cell_serializer`]: crate::Column::cell_serializer
/// [`Column::as_cell_seed`]: crate::Column::as_cell_seed
#[derive(Debug, Clone, PartialEq)]
pub enum Cell<'b> {
    /// The cell only contains a single [`Value`]. This is the only supported type
    /// in [`BdatVersion::Modern`] BDATs.
    Single(Value<'b>),
    /// The cell contains a list of [`Value`]s
    List(Vec<Value<'b>>),
    /// The cell acts as a list of integers, derived by masking bits from the
    /// parent value.
    Flags(Vec<u32>),
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
pub enum Value<'b> {
    Unknown,
    UnsignedByte(u8),
    UnsignedShort(u16),
    UnsignedInt(u32),
    SignedByte(i8),
    SignedShort(i16),
    SignedInt(i32),
    String(Utf<'b>),
    Float(BdatReal),
    /// A hash referencing a row in the same or some other table
    HashRef(u32),
    Percent(u8),
    /// It points to a (generally empty) string in the string table,
    /// mostly used for `DebugName` fields.
    DebugString(Utf<'b>),
    /// [`BdatVersion::Modern`] unknown type (0xc)
    Unknown2(u8),
    /// [`BdatVersion::Modern`] unknown type (0xd)
    /// It seems to be some sort of translation index, mostly used for
    /// `Name` and `Caption` fields.
    Unknown3(u16),
}

/// An optionally-borrowed clone-on-write UTF-8 string.
pub type Utf<'t> = Cow<'t, str>;

/// Modern is currently an alias for the value contained in single-value cells,
/// which is the only cell type supported by the modern format.
pub type ModernCell<'t, 'tb> = &'t Value<'tb>;

/// Legacy is currently an alias for the version-agnostic [`Cell`].
pub type LegacyCell<'t, 'tb> = &'t Cell<'tb>;

pub trait FromValue<'t, 'tb>
where
    Self: Sized,
{
    fn extract(value: &'t Value<'tb>) -> Option<Self>;
}

/// Converts from a cell reference.
pub trait FromCell<'t, 'tb> {
    /// Converts from a cell reference.
    ///
    /// The implementation can panic.
    fn from_cell(cell: &'t Cell<'tb>) -> Self;
}

impl<'b> Cell<'b> {
    /// Gets a reference to the cell's value, if it
    /// is a [`Cell::Single`], and returns [`None`] otherwise.
    pub fn as_single(&self) -> Option<&Value> {
        match self {
            Self::Single(v) => Some(v),
            _ => None,
        }
    }

    /// Consumes the cell and returns its value, if it is a [`Cell::Single`],
    /// or [`None`] otherwise.
    pub fn into_single(self) -> Option<Value<'b>> {
        match self {
            Self::Single(v) => Some(v),
            _ => None,
        }
    }

    /// Gets a reference to the cell's list of values, if it
    /// is a [`Cell::List`], and returns [`None`] otherwise.
    pub fn as_list(&self) -> Option<&[Value<'b>]> {
        match self {
            Self::List(v) => Some(v),
            _ => None,
        }
    }

    /// Consumes the cell and returns its list of values, if it is a [`Cell::List`],
    /// or [`None`] otherwise.
    pub fn into_list(self) -> Option<Vec<Value<'b>>> {
        match self {
            Self::List(v) => Some(v),
            _ => None,
        }
    }

    /// Gets a reference to the cell's list of flag values, if it
    /// is a [`Cell::Flags`], and returns [`None`] otherwise.
    pub fn as_flags(&self) -> Option<&[u32]> {
        match self {
            Self::Flags(v) => Some(v),
            _ => None,
        }
    }

    /// Consumes the cell and returns its list of flag values, if it is a [`Cell::Flags`],
    /// or [`None`] otherwise.
    pub fn into_flags(self) -> Option<Vec<u32>> {
        match self {
            Self::Flags(v) => Some(v),
            _ => None,
        }
    }
}

impl<'b> Value<'b> {
    /// Casts the underlying value to `V`.
    ///
    /// For strings, the [`Utf`] type alias, or `&str` can be used.
    ///
    /// ## Panics
    /// Panics if the value's internal type is not `V`. The type must match
    /// exactly, e.g. `i32` is not the same as `u32`.
    ///
    /// To convert instead of panicking, use [`to_integer`], [`to_float`], etc.
    ///
    /// [`to_integer`]: Value::to_integer
    /// [`to_float`]: Value::to_float
    pub fn get_as<'t, V: FromValue<'t, 'b>>(&'t self) -> V {
        self.try_get_as().unwrap()
    }

    /// Attempts to cast the underlying value to `V`.
    ///
    /// For strings, the [`Utf`] type alias, or `&str` can be used.
    ///
    /// Fails if the value's internal type is not `V`. The type must match
    /// exactly, e.g. `i32` is not the same as `u32`.
    ///
    /// To convert instead of failing, use [`to_integer`], [`to_float`], etc.
    ///
    /// [`to_integer`]: Value::to_integer
    /// [`to_float`]: Value::to_float
    pub fn try_get_as<'t, V: FromValue<'t, 'b>>(&'t self) -> BdatResult<V> {
        V::extract(self).ok_or_else(|| BdatError::ValueCast(self.into()))
    }

    /// Returns the integer representation of this value.
    /// For signed values, this is the unsigned representation.
    ///
    /// # Panics
    /// If the value is not stored as an integer.
    /// Do not use this for floats, use [`Value::to_float`] instead.
    pub fn to_integer(&self) -> u32 {
        match self {
            Self::SignedByte(b) => *b as u32,
            Self::Percent(b) | Self::UnsignedByte(b) | Self::Unknown2(b) => *b as u32,
            Self::SignedShort(s) => *s as u32,
            Self::UnsignedShort(s) | Self::Unknown3(s) => *s as u32,
            Self::SignedInt(i) => *i as u32,
            Self::UnsignedInt(i) | Self::HashRef(i) => *i,
            _ => panic!("value is not an integer"),
        }
    }

    /// Returns the floating point representation of this value.
    ///
    /// # Panics
    /// If the value is not stored as a float.
    pub fn to_float(&self) -> f32 {
        match self {
            Self::Float(f) => (*f).into(),
            _ => panic!("value is not a float"),
        }
    }

    /// Returns the underlying string value.
    /// This does **not** format other values, use the Display trait for that.
    ///
    /// **Note**: this will potentially copy the string, if the table is borrowing its source.
    /// To avoid copies, use [`Value::as_str`].
    ///
    /// # Panics
    /// If the value is not stored as a string.
    pub fn into_string(self) -> String {
        match self {
            Self::String(s) | Self::DebugString(s) => s.into_owned(),
            _ => panic!("value is not a string"),
        }
    }

    /// Returns a reference to the underlying string value.
    ///
    /// # Panics
    /// If the value is not stored as a string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::String(s) | Self::DebugString(s) => s.as_ref(),
            _ => panic!("value is not a string"),
        }
    }
}

impl ValueType {
    /// Returns the size of a single cell with this value type.
    pub fn data_len(self) -> usize {
        use ValueType::*;
        match self {
            Unknown => 0,
            UnsignedByte | SignedByte | Percent | Unknown2 => 1,
            UnsignedShort | SignedShort | Unknown3 => 2,
            UnsignedInt | SignedInt | String | Float | HashRef | DebugString => 4,
        }
    }

    /// Returns whether the given version supports the value type.
    pub fn is_supported(self, version: BdatVersion) -> bool {
        use ValueType::*;
        match self {
            Percent | Unknown2 | Unknown3 | HashRef | DebugString => version == BdatVersion::Modern,
            _ => true,
        }
    }
}

impl From<ValueType> for u8 {
    fn from(t: ValueType) -> Self {
        t as u8
    }
}

impl<'t, 'tb> FromCell<'t, 'tb> for ModernCell<'t, 'tb> {
    fn from_cell(cell: &'t Cell<'tb>) -> Self {
        match cell {
            Cell::Single(v) => v,
            _ => panic!("only Cell::Single supported in modern bdats"),
        }
    }
}

impl<'t, 'tb: 't, T> FromCell<'t, 'tb> for T
where
    T: From<&'t Cell<'tb>>,
{
    fn from_cell(cell: &'t Cell<'tb>) -> Self {
        Self::from(cell)
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

impl<'b> Display for Value<'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => Ok(()),
            Self::HashRef(h) => Label::Hash(*h).fmt(f),
            Self::Percent(v) => write!(f, "{}%", v),
            v => {
                default_display!(f, v, SignedByte SignedShort SignedInt UnsignedByte UnsignedShort UnsignedInt DebugString Unknown2 Unknown3 String Float)
            }
        }
    }
}

impl<'b> Display for Cell<'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Single(val) => val.fmt(f),
            Cell::List(list) => {
                write!(f, "[")?;
                for (i, value) in list.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    value.fmt(f)?;
                }
                write!(f, "]")
            }
            Cell::Flags(nums) => {
                write!(f, "{{")?;
                for (i, value) in nums.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    value.fmt(f)?;
                }
                write!(f, "}}")
            }
        }
    }
}

macro_rules! from_value {
    ($val:ty, $($variants:path ) *) => {
        impl<'t, 'tb> FromValue<'t, 'tb> for $val {
            fn extract(value: &Value<'_>) -> Option<Self> {
                match value {
                    $( | $variants(v) )* => Some(*v),
                    _ => None,
                }
            }
        }
    };
}

from_value!(u32, Value::UnsignedInt Value::HashRef);
from_value!(u16, Value::UnsignedShort Value::Unknown3);
from_value!(u8, Value::UnsignedByte Value::Unknown2 Value::Percent);
from_value!(i32, Value::SignedInt);
from_value!(i16, Value::SignedShort);
from_value!(i8, Value::SignedByte);
from_value!(BdatReal, Value::Float);

impl<'t, 'tb> FromValue<'t, 'tb> for f32 {
    fn extract(value: &Value<'_>) -> Option<Self> {
        BdatReal::extract(value).map(Into::into)
    }
}

impl<'t, 'tb> FromValue<'t, 'tb> for Utf<'tb> {
    fn extract(value: &Value<'tb>) -> Option<Self> {
        match value {
            Value::String(s) | Value::DebugString(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl<'t, 'tb> FromValue<'t, 'tb> for &'t str {
    fn extract(value: &'t Value<'tb>) -> Option<Self> {
        match value {
            Value::String(s) | Value::DebugString(s) => Some(s.as_ref()),
            _ => None,
        }
    }
}
