use std::fmt::{Display, Formatter};

use crate::{BdatVersion, LegacyVersion};

/// A real number with a different internal representation based on the BDAT version.
///
/// This type implements `Into<f32>` to extract the correct floating-point value.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub enum BdatReal {
    Floating(IeeeFloat),
    Fixed(CrossFixed),
    Unknown(f32),
}

/// IEEE-754 floating point, used in XC1/2/DE legacy BDATs, and in modern BDATs
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct IeeeFloat(f32);

/// Base 4096 fixed-point decimal, used in XCX legacy BDATs
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct CrossFixed(f32);

impl BdatReal {
    /// Converts a 32-bit representation into the proper real value, based on
    /// the BDAT version.
    pub fn from_bits(bits: u32, version: BdatVersion) -> Self {
        match version {
            BdatVersion::Legacy(LegacyVersion::X) => Self::Fixed(bits.into()),
            _ => Self::Floating(f32::from_bits(bits).into()),
        }
    }

    /// Converts the real value back to a 32-bit representation.
    ///
    /// ## Panics
    /// Panics if no version is attached to the real value, i.e. it is a `BdatReal::Unknown`.
    pub fn to_bits(&self) -> u32 {
        match self {
            Self::Floating(f) => f.0.to_bits(),
            Self::Fixed(f) => u32::from(*f),
            _ => panic!("unknown real bit conversion"),
        }
    }

    /// Converts the underlying real number into either a floating-point or a fixed-point
    /// representation.
    ///
    /// Does nothing if `self` is not [`BdatReal::Unknown`].
    pub fn make_known(&mut self, version: BdatVersion) {
        let Self::Unknown(internal) = *self else {
            return;
        };
        match version {
            BdatVersion::Legacy(LegacyVersion::X) => *self = Self::Fixed(internal.into()),
            _ => *self = Self::Floating(internal.into()),
        }
    }
}

impl From<IeeeFloat> for f32 {
    fn from(value: IeeeFloat) -> Self {
        value.0
    }
}

impl From<f32> for IeeeFloat {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<CrossFixed> for f32 {
    fn from(value: CrossFixed) -> Self {
        value.0
    }
}

impl From<f32> for CrossFixed {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<u32> for CrossFixed {
    fn from(value: u32) -> Self {
        Self((value as f64 / 4096.0) as f32)
    }
}

impl From<CrossFixed> for u32 {
    fn from(value: CrossFixed) -> u32 {
        (value.0 as f64 * 4096.0) as u32
    }
}

impl From<BdatReal> for f32 {
    fn from(value: BdatReal) -> Self {
        match value {
            BdatReal::Floating(f) => f.into(),
            BdatReal::Fixed(f) => f.into(),
            BdatReal::Unknown(f) => f,
        }
    }
}

#[cfg(test)]
impl From<f32> for BdatReal {
    fn from(value: f32) -> Self {
        Self::Unknown(value)
    }
}

impl Display for BdatReal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f32::from(*self).fmt(f)
    }
}
