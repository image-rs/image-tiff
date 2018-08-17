use std::error::Error;
use std::fmt;
use std::io;
use std::string;

/// Tiff error kinds.
#[derive(Debug)]
pub enum TiffError {
    /// The Image is not formatted properly
    FormatError(String),

    /// The Decoder does not support this image format
    UnsupportedError(String),

    /// An I/O Error occurred while decoding the image
    IoError(io::Error),
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

impl From<string::FromUtf8Error> for TiffError {
    fn from(err: string::FromUtf8Error) -> TiffError {
        TiffError::FormatError(String::from("Image contains invalid tag."))
    }
}

/// Result of an image decoding/encoding process
pub type TiffResult<T> = Result<T, TiffError>;
