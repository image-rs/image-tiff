use std::error::Error;
use std::fmt;
use std::io;
use std::str;
use std::string;

use jpeg::UnsupportedFeature;
use quick_error::quick_error;

use crate::decoder::ChunkType;
use crate::tags::{
    CompressionMethod, PhotometricInterpretation, PlanarConfiguration, SampleFormat, Tag,
};
use crate::ColorType;

use crate::weezl::LzwError;

/// Tiff error kinds.
#[derive(Debug)]
pub enum TiffError {
    /// The Image is not formatted properly.
    FormatError(TiffFormatError),

    /// The Decoder does not support features required by the image.
    UnsupportedError(TiffUnsupportedError),

    /// An I/O Error occurred while decoding the image.
    IoError(io::Error),

    /// The Limits of the Decoder is exceeded.
    LimitsExceeded,

    /// An integer conversion to or from a platform size failed, either due to
    /// limits of the platform size or limits of the format.
    IntSizeError,

    /// The image does not support the requested operation
    UsageError(UsageError),
}

quick_error! {
    /// The image is not formatted properly.
    ///
    /// This indicates that the encoder producing the image might behave incorrectly or that the
    /// input file has been corrupted.
    ///
    /// The list of variants may grow to incorporate errors of future features. Matching against
    /// this exhaustively is not covered by interface stability guarantees.
    #[derive(Debug, Clone, PartialEq)]
    #[non_exhaustive]
    pub enum TiffFormatError {
        TiffSignatureNotFound {
            display("TIFF signature not found.")
        }
        TiffSignatureInvalid {
            display("TIFF signature invalid.")
        }
        ImageFileDirectoryNotFound {
            display("Image file directory not found.")
        }
        InconsistentSizesEncountered {
            display("Inconsistent sizes encountered.")
        }
        InvalidDimensions(width: u32, height: u32) {
            display("Invalid dimensions: {width}x{height}.")
        }
        InvalidTag {
            display("Image contains invalid tag.")
        }
        InvalidTagValueType(tag: Tag) {
            display("Tag `{:?}` did not have the expected value type.", tag)
        }
        RequiredTagNotFound(tag: Tag) {
            display("Required tag `{:?}` not found.", tag)
        }
        UnknownPredictor(pred: u16) {
            display("Unknown predictor “{}” encountered", pred)
        }
        UnknownPlanarConfiguration(cfg: u16) {
            display("Unknown planar configuration “{}” encountered", cfg)
        }
        InvalidTypeForTag {
            display("Tag has invalid type.")
        }
        StripTileTagConflict {
            display("File should contain either (StripByteCounts and StripOffsets) or (TileByteCounts and TileOffsets), other combination was found.")
        }
        CycleInOffsets {
            display("File contained a cycle in the list of IFDs")
        }
        SamplesPerPixelIsZero {
            display("Samples per pixel is zero")
        }
        CompressedDataCorrupt(message: String) {
            display("Compressed data is corrupt: {message}")
        }
    }
}

/// The Decoder does not support features required by the image.
///
/// This only captures known failures for which the standard either does not require support or an
/// implementation has been planned but not yet completed. Some variants may become unused over
/// time and will then get deprecated before being removed.
///
/// The list of variants may grow. Matching against this exhaustively is not covered by interface
/// stability guarantees.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TiffUnsupportedError {
    FloatingPointPredictor(ColorType),
    HorizontalPredictor(ColorType),
    InconsistentBitsPerSample(Vec<u8>),
    InterpretationWithBits(PhotometricInterpretation, Vec<u8>),
    UnknownInterpretation,
    UnknownCompressionMethod,
    UnsupportedCompressionMethod(CompressionMethod),
    UnsupportedSampleDepth(u8),
    UnsupportedSampleFormat(Vec<SampleFormat>),
    UnsupportedColorType(ColorType),
    UnsupportedBitsPerChannel(u8),
    UnsupportedPlanarConfig(Option<PlanarConfiguration>),
    UnsupportedDataType,
    UnsupportedInterpretation(PhotometricInterpretation),
    UnsupportedJpegFeature(UnsupportedFeature),
    MisalignedTileBoundaries,
}

impl fmt::Display for TiffUnsupportedError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        use self::TiffUnsupportedError::*;
        match *self {
            FloatingPointPredictor(color_type) => write!(
                fmt,
                "Floating point predictor for {:?} is unsupported.",
                color_type
            ),
            HorizontalPredictor(color_type) => write!(
                fmt,
                "Horizontal predictor for {:?} is unsupported.",
                color_type
            ),
            InconsistentBitsPerSample(ref bits_per_sample) => {
                write!(fmt, "Inconsistent bits per sample: {:?}.", bits_per_sample)
            }
            InterpretationWithBits(ref photometric_interpretation, ref bits_per_sample) => write!(
                fmt,
                "{:?} with {:?} bits per sample is unsupported",
                photometric_interpretation, bits_per_sample
            ),
            UnknownInterpretation => write!(
                fmt,
                "The image is using an unknown photometric interpretation."
            ),
            UnknownCompressionMethod => write!(fmt, "Unknown compression method."),
            UnsupportedCompressionMethod(method) => {
                write!(fmt, "Compression method {:?} is unsupported", method)
            }
            UnsupportedSampleDepth(samples) => {
                write!(fmt, "{} samples per pixel is unsupported.", samples)
            }
            UnsupportedSampleFormat(ref formats) => {
                write!(fmt, "Sample format {:?} is unsupported.", formats)
            }
            UnsupportedColorType(color_type) => {
                write!(fmt, "Color type {:?} is unsupported", color_type)
            }
            UnsupportedBitsPerChannel(bits) => {
                write!(fmt, "{} bits per channel not supported", bits)
            }
            UnsupportedPlanarConfig(config) => {
                write!(fmt, "Unsupported planar configuration “{:?}”.", config)
            }
            UnsupportedDataType => write!(fmt, "Unsupported data type."),
            UnsupportedInterpretation(interpretation) => {
                write!(
                    fmt,
                    "Unsupported photometric interpretation \"{:?}\".",
                    interpretation
                )
            }
            UnsupportedJpegFeature(ref unsupported_feature) => {
                write!(fmt, "Unsupported JPEG feature {:?}", unsupported_feature)
            }
            MisalignedTileBoundaries => write!(fmt, "Tile rows are not aligned to byte boundaries"),
        }
    }
}

/// User attempted to use the Decoder in a way that is incompatible with a specific image.
///
/// For example: attempting to read a tile from a stripped image.
#[derive(Debug)]
pub enum UsageError {
    InvalidChunkType(ChunkType, ChunkType),
    InvalidChunkIndex(u32),
    PredictorCompressionMismatch,
    PredictorIncompatible,
    PredictorUnavailable,
}

impl fmt::Display for UsageError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::UsageError::*;
        match *self {
            InvalidChunkType(expected, actual) => {
                write!(
                    fmt,
                    "Requested operation is only valid for images with chunk encoding of type: {:?}, got {:?}.",
                    expected, actual
                )
            }
            InvalidChunkIndex(index) => write!(fmt, "Image chunk index ({}) requested.", index),
            PredictorCompressionMismatch => write!(
                fmt,
                "The requested predictor is not compatible with the requested compression"
            ),
            PredictorIncompatible => write!(
                fmt,
                "The requested predictor is not compatible with the image's format"
            ),
            PredictorUnavailable => write!(fmt, "The requested predictor is not available"),
        }
    }
}

impl fmt::Display for TiffError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            TiffError::FormatError(ref e) => write!(fmt, "Format error: {}", e),
            TiffError::UnsupportedError(ref f) => write!(
                fmt,
                "The Decoder does not support the \
                 image format `{}`",
                f
            ),
            TiffError::IoError(ref e) => e.fmt(fmt),
            TiffError::LimitsExceeded => write!(fmt, "The Decoder limits are exceeded"),
            TiffError::IntSizeError => write!(fmt, "Platform or format size limits exceeded"),
            TiffError::UsageError(ref e) => write!(fmt, "Usage error: {}", e),
        }
    }
}

impl Error for TiffError {
    fn description(&self) -> &str {
        match *self {
            TiffError::FormatError(..) => "Format error",
            TiffError::UnsupportedError(..) => "Unsupported error",
            TiffError::IoError(..) => "IO error",
            TiffError::LimitsExceeded => "Decoder limits exceeded",
            TiffError::IntSizeError => "Platform or format size limits exceeded",
            TiffError::UsageError(..) => "Invalid usage",
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            TiffError::IoError(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for TiffError {
    fn from(err: io::Error) -> TiffError {
        TiffError::IoError(err)
    }
}

impl From<str::Utf8Error> for TiffError {
    fn from(_err: str::Utf8Error) -> TiffError {
        TiffError::FormatError(TiffFormatError::InvalidTag)
    }
}

impl From<string::FromUtf8Error> for TiffError {
    fn from(_err: string::FromUtf8Error) -> TiffError {
        TiffError::FormatError(TiffFormatError::InvalidTag)
    }
}

impl From<TiffFormatError> for TiffError {
    fn from(err: TiffFormatError) -> TiffError {
        TiffError::FormatError(err)
    }
}

impl From<TiffUnsupportedError> for TiffError {
    fn from(err: TiffUnsupportedError) -> TiffError {
        TiffError::UnsupportedError(err)
    }
}

impl From<UsageError> for TiffError {
    fn from(err: UsageError) -> TiffError {
        TiffError::UsageError(err)
    }
}

impl From<std::num::TryFromIntError> for TiffError {
    fn from(_err: std::num::TryFromIntError) -> TiffError {
        TiffError::IntSizeError
    }
}

impl From<LzwError> for TiffError {
    fn from(err: LzwError) -> TiffError {
        TiffError::FormatError(TiffFormatError::CompressedDataCorrupt(err.to_string()))
    }
}

impl From<jpeg::Error> for TiffError {
    fn from(err: jpeg::Error) -> Self {
        TiffError::FormatError(TiffFormatError::CompressedDataCorrupt(err.to_string()))
    }
}

/// Result of an image decoding/encoding process
pub type TiffResult<T> = Result<T, TiffError>;
