use crate::table::FormatConvertError;
use crate::{BdatVersion, DetectError, ValueType, Label};
use std::num::TryFromIntError;
use std::str::Utf8Error;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, BdatError>;

#[derive(Error, Debug)]
pub enum BdatError {
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Malformed BDAT ({0:?})")]
    MalformedBdat(Scope),
    #[error(transparent)]
    InvalidLength(#[from] TryFromIntError),
    #[error("Unknown cell type: {0}")]
    UnknownCellType(u8),
    #[error("Unknown value type: {0}")]
    UnknownValueType(u8),
    #[error("Unsupported type: BDAT version {1:?} does not support value type {0:?}")]
    UnsupportedType(ValueType, BdatVersion),
    #[error("Invalid flag type: value type {0:?} does not support flags")]
    InvalidFlagType(ValueType),
    #[error("Could not detect version: {0}")]
    VersionDetect(#[from] DetectError),
    #[error("Could not convert table: {0}")]
    FormatConvert(#[from] FormatConvertError),
    #[error("Unsupported cast type for {0:?}")]
    ValueCast(ValueType),
    #[error(
        "Duplicate hash key ({}: {}) in rows {} and {}. Duplicate keys are not allowed in the primary key table.",
        _0.0, _0.1, _0.2, _0.3
    )]
    DuplicateKey(Box<(Label, Label, usize, usize)>),
}

#[derive(Debug)]
pub enum Scope {
    Table,
    File,
}
