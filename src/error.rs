use std::error::Error;
use std::fmt;
use std::io;
use std::string;

use decoder::ifd::{Tag, Value};
use decoder::{PhotometricInterpretation, CompressionMethod, PlanarConfiguration};
use ColorType;

/// Tiff error kinds.
#[derive(Debug)]
pub enum TiffError {
    /// The Image is not formatted properly
    FormatError(TiffFormatError),

    /// The Decoder does not support this image format
    UnsupportedError(TiffUnsupportedError),

    /// An I/O Error occurred while decoding the image
    IoError(io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TiffFormatError {
    TiffSignatureNotFound,
    TiffSignatureInvalid,
    ImageFileDirectoryNotFound,
    InconsistentSizesEncountered,
    InvalidTag,
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
            InconsistentSizesEncountered => write!(fmt, "Inconsistent sizes encountered."),
            InvalidTag => write!(fmt, "Image contains invalid tag."),
            RequiredTagNotFound(ref tag) => write!(fmt, "Required tag `{:?}` not found.", tag),
            UnknownPredictor(ref predictor) => write!(fmt, "Unknown predictor “{}” encountered", predictor),
            UnsignedIntegerExpected(ref val) => write!(fmt,  "Expected unsigned integer, {:?} found.", val),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TiffUnsupportedError {
    HorizontalPredictor(ColorType),
    InterpretationWithBits(PhotometricInterpretation, Vec<u8>),
    UnknownInterpretation,
    UnknownCompressionMethod,
    UnsupportedCompressionMethod(CompressionMethod),
    UnsupportedSampleDepth(u8),
    UnsupportedColorType(ColorType),
    UnsupportedBitsPerChannel(u8),
    UnsupportedPlanarConfig(Option<PlanarConfiguration>),
    UnsupportedDataType,
}

impl fmt::Display for TiffUnsupportedError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        use self::TiffUnsupportedError::*;
        match *self {
            HorizontalPredictor(color_type) => write!(fmt, "Horizontal predictor for {:?} is unsupported.", color_type),
            InterpretationWithBits(ref photometric_interpretation, ref bits_per_sample) => {
                write!(fmt, "{:?} with {:?} bits per sample is unsupported", photometric_interpretation, bits_per_sample)
            },
            UnknownInterpretation => write!(fmt, "The image is using an unknown photometric interpretation."),
            UnknownCompressionMethod => write!(fmt, "Unknown compression method."),
            UnsupportedCompressionMethod(method) => write!(fmt, "Compression method {:?} is unsupported", method),
            UnsupportedSampleDepth(samples) => write!(fmt, "{} samples per pixel is supported.", samples),
            UnsupportedColorType(color_type) => write!(fmt, "Color type {:?} is unsupported", color_type),
            UnsupportedBitsPerChannel(bits) => write!(fmt, "{} bits per channel not supported", bits),
            UnsupportedPlanarConfig(config) => write!(fmt, "Unsupported planar configuration “{:?}”.", config),
            UnsupportedDataType => write!(fmt, "Unsupported data type."),
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

impl From<string::FromUtf8Error> for TiffError {
    fn from(_err: string::FromUtf8Error) -> TiffError {
        TiffError::FormatError(TiffFormatError::InvalidTag)
    }
}

/// Result of an image decoding/encoding process
pub type TiffResult<T> = Result<T, TiffError>;
