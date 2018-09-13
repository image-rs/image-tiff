use std::error::Error;
use std::fmt;
use std::io;

use decoder::ifd::{Tag, Value};

/// Tiff error kinds.
#[derive(Debug)]
pub enum TiffError {
    /// The Image is not formatted properly
    FormatError(TiffFormatError),

    /// The Decoder does not support this image format
    UnsupportedError(String),

    /// An I/O Error occurred while decoding the image
    IoError(io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TiffFormatError {
    TiffSignatureNotFound,
    TiffSignatureInvalid,
    ImageFileDirectoryNotFound,
    RequiredTagNotFound(Tag),
    UnknownPredictor(u32),
    UnsignedIntegerExpected(Value),
}

impl fmt::Display for TiffFormatError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        use self::TiffFormatError::*;
        match *self {
            TiffSignatureNotFound => write!(fmt, "TIFF signature not found."),
            TiffSignatureInvalid => write!(fmt, "TIFF signature invalid."),
            ImageFileDirectoryNotFound => write!(fmt, "Image file directory not found."),
            RequiredTagNotFound(ref tag) => write!(fmt, "Required tag `{:?}` not found.", tag),
            UnknownPredictor(ref predictor) => write!(fmt, "Unknown predictor “{}” encountered", predictor),
            UnsignedIntegerExpected(ref val) => write!(fmt,  "Expected unsigned integer, {:?} found.", val),
        }
    }
}

impl fmt::Display for TiffError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            TiffError::FormatError(ref e) => write!(fmt, "Format error: {}", e),
            TiffError::UnsupportedError(ref f) => write!(fmt, "The Decoder does not support the \
                                                                 image format `{}`", f),
            TiffError::IoError(ref e) => e.fmt(fmt),
        }
    }
}

impl Error for TiffError {
    fn description (&self) -> &str {
        match *self {
            TiffError::FormatError(..) => "Format error",
            TiffError::UnsupportedError(..) => "Unsupported error",
            TiffError::IoError(..) => "IO error",
        }
    }

    fn cause (&self) -> Option<&Error> {
        match *self {
            TiffError::IoError(ref e) => Some(e),
            _ => None
        }
    }
}

impl From<io::Error> for TiffError {
    fn from(err: io::Error) -> TiffError {
        TiffError::IoError(err)
    }
}

/// Result of an image decoding/encoding process
pub type TiffResult<T> = Result<T, TiffError>;
