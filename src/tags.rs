macro_rules! tags {
    {
        // Permit arbitrary meta items, which include documentation.
        $( #[$enum_attr:meta] )*
        $vis:vis enum $name:ident($ty:tt) $(unknown($unknown_doc:literal))* {
            // Each of the `Name = Val,` permitting documentation.
            $($(#[$ident_attr:meta])* $tag:ident = $val:expr,)*
        }
    } => {
        $( #[$enum_attr] )*
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $($(#[$ident_attr])* $tag,)*
            $(
                #[doc = $unknown_doc]
                Unknown($ty),
            )*
        }

        impl $name {
            #[inline(always)]
            fn __from_inner_type(n: $ty) -> Result<Self, $ty> {
                match n {
                    $( $val => Ok($name::$tag), )*
                    n => Err(n),
                }
            }

            #[inline(always)]
            fn __to_inner_type(&self) -> $ty {
                match *self {
                    $( $name::$tag => $val, )*
                    $( $name::Unknown(n) => { $unknown_doc; n }, )*
                }
            }
        }

        tags!($name, $ty, $($unknown_doc)*);
    };
    // For u16 tags, provide direct inherent primitive conversion methods.
    ($name:tt, u16, $($unknown_doc:literal)*) => {
        impl $name {
            #[inline(always)]
            pub fn from_u16(val: u16) -> Option<Self> {
                Self::__from_inner_type(val).ok()
            }

            $(
            #[inline(always)]
            pub fn from_u16_exhaustive(val: u16) -> Self {
                $unknown_doc;
                Self::__from_inner_type(val).unwrap_or_else(|_| $name::Unknown(val))
            }
            )*

            #[inline(always)]
            pub fn to_u16(&self) -> u16 {
                Self::__to_inner_type(self)
            }
        }
    };
    // For other tag types, do nothing for now. With concat_idents one could
    // provide inherent conversion methods for all types.
    ($name:tt, $ty:tt, $($unknown_doc:literal)*) => {};
}

// Note: These tags appear in the order they are mentioned in the TIFF reference
tags! {
/// TIFF tags
#[derive(Ord, PartialOrd)]
pub enum Tag(u16) unknown("A private or extension tag") {
    // Baseline tags:
    Artist = 315,
    // grayscale images PhotometricInterpretation 1 or 3
    BitsPerSample = 258,
    CellLength = 265, // TODO add support
    CellWidth = 264, // TODO add support
    // palette-color images (PhotometricInterpretation 3)
    ColorMap = 320, // TODO add support
    Compression = 259, // TODO add support for 2 and 32773
    DateTime = 306,
    ExtraSamples = 338, // TODO add support
    FillOrder = 266, // TODO add support
    FreeByteCounts = 289, // TODO add support
    FreeOffsets = 288, // TODO add support
    GrayResponseCurve = 291, // TODO add support
    GrayResponseUnit = 290, // TODO add support
    HostComputer = 316,
    ImageDescription = 270,
    ImageLength = 257,
    ImageWidth = 256,
    Make = 271,
    MaxSampleValue = 281, // TODO add support
    MinSampleValue = 280, // TODO add support
    Model = 272,
    NewSubfileType = 254, // TODO add support
    Orientation = 274, // TODO add support
    PhotometricInterpretation = 262,
    PlanarConfiguration = 284,
    ResolutionUnit = 296, // TODO add support
    RowsPerStrip = 278,
    SamplesPerPixel = 277,
    Software = 305,
    StripByteCounts = 279,
    StripOffsets = 273,
    SubfileType = 255, // TODO add support
    Threshholding = 263, // TODO add support
    XResolution = 282,
    YResolution = 283,
    // Advanced tags
    Predictor = 317,
    TileWidth = 322,
    TileLength = 323,
    TileOffsets = 324,
    TileByteCounts = 325,
    // Data Sample Format
    SampleFormat = 339,
    SMinSampleValue = 340, // TODO add support
    SMaxSampleValue = 341, // TODO add support
    // JPEG
    JPEGTables = 347,
    // GeoTIFF
    ModelPixelScaleTag = 33550, // (SoftDesk)
    ModelTransformationTag = 34264, // (JPL Carto Group)
    ModelTiepointTag = 33922, // (Intergraph)
    GeoKeyDirectoryTag = 34735, // (SPOT)
    GeoDoubleParamsTag = 34736, // (SPOT)
    GeoAsciiParamsTag = 34737, // (SPOT)
    Copyright = 0x8298,
    ExifIfd = 0x8769,
    GpsIfd = 0x8825,
    ISO = 0x8827,
    ExifVersion = 0x9000,
    DateTimeOriginal = 0x9003,
    CreateDate = 0x9004,
    ComponentsConfiguration = 0x9101,
    UserComment = 0x9286,
    GdalNodata = 42113, // Contains areas with missing data
    FlaspixVersion = 0xa000,
    InteropIfd = 0xa005,
}
}

tags! {
/// Tag space of GPS ifds
#[derive(Ord, PartialOrd)]
pub enum GpsTag(u16) unknown("A private or extension tag") {
    GPSVersionID = 0x0000,
    GPSLatitudeRef = 0x0001,
    GPSLatitude = 0x0002,
    GPSLongitudeRef = 0x0003,
    GPSLongitude = 0x0004,
    GPSAltitudeRef = 0x0005,
    GPSAltitude = 0x0006,
    GPSTimeStamp = 0x0007,
    GPSSatellites = 0x0008,
    GPSStatus = 0x0009,
    GPSMeasureMode = 0x000a,
    GPSDOP = 0x000b,
    GPSSpeedRef = 0x000c,
    GPSSpeed = 0x000d,
    GPSTrackRef = 0x000e,
    GPSTrack = 0x000f,
    GPSImgDirectionRef = 0x0010,
    GPSImgDirection = 0x0011,
    GPSMapDatum = 0x0012,
    GPSDestLatitudeRef = 0x0013,
    GPSDestLatitude = 0x0014,
    GPSDestLongitudeRef = 0x0015,
    GPSDestLongitude = 0x0016,
    GPSDestBearingRef = 0x0017,
    GPSDestBearing = 0x0018,
    GPSDestDistanceRef = 0x0019,
    GPSDestDistance = 0x001a,
    GPSProcessingMethod = 0x001b,
    GPSAreaInformation = 0x001c,
    GPSDateStamp = 0x001d,
    GPSDifferential = 0x001e,
    GPSHPositioningError = 0x001f,
}
}

tags! {
/// The type of an IFD entry (a 2 byte field).
pub enum Type(u16) {
    /// 8-bit unsigned integer
    BYTE = 1,
    /// 8-bit byte that contains a 7-bit ASCII code; the last byte must be zero
    ASCII = 2,
    /// 16-bit unsigned integer
    SHORT = 3,
    /// 32-bit unsigned integer
    LONG = 4,
    /// Fraction stored as two 32-bit unsigned integers
    RATIONAL = 5,
    /// 8-bit signed integer
    SBYTE = 6,
    /// 8-bit byte that may contain anything, depending on the field
    UNDEFINED = 7,
    /// 16-bit signed integer
    SSHORT = 8,
    /// 32-bit signed integer
    SLONG = 9,
    /// Fraction stored as two 32-bit signed integers
    SRATIONAL = 10,
    /// 32-bit IEEE floating point
    FLOAT = 11,
    /// 64-bit IEEE floating point
    DOUBLE = 12,
    /// 32-bit unsigned integer (offset)
    IFD = 13,
    /// BigTIFF 64-bit unsigned integer
    LONG8 = 16,
    /// BigTIFF 64-bit signed integer
    SLONG8 = 17,
    /// BigTIFF 64-bit unsigned integer (offset)
    IFD8 = 18,
}
}

tags! {
/// See [TIFF compression tags](https://www.awaresystems.be/imaging/tiff/tifftags/compression.html)
/// for reference.
pub enum CompressionMethod(u16) unknown("A custom compression method") {
    None = 1,
    Huffman = 2,
    Fax3 = 3,
    Fax4 = 4,
    LZW = 5,
    JPEG = 6,
    // "Extended JPEG" or "new JPEG" style
    ModernJPEG = 7,
    Deflate = 8,
    OldDeflate = 0x80B2,
    PackBits = 0x8005,

    // Self-assigned by libtiff
    ZSTD = 0xC350,
}
}

tags! {
pub enum PhotometricInterpretation(u16) {
    WhiteIsZero = 0,
    BlackIsZero = 1,
    RGB = 2,
    RGBPalette = 3,
    TransparencyMask = 4,
    CMYK = 5,
    YCbCr = 6,
    CIELab = 8,
}
}

tags! {
pub enum PlanarConfiguration(u16) {
    Chunky = 1,
    Planar = 2,
}
}

tags! {
pub enum Predictor(u16) {
    /// No changes were made to the data
    None = 1,
    /// The images' rows were processed to contain the difference of each pixel from the previous one.
    ///
    /// This means that instead of having in order `[r1, g1. b1, r2, g2 ...]` you will find
    /// `[r1, g1, b1, r2-r1, g2-g1, b2-b1, r3-r2, g3-g2, ...]`
    Horizontal = 2,
    /// Not currently supported
    FloatingPoint = 3,
}
}

tags! {
/// Type to represent resolution units
pub enum ResolutionUnit(u16) {
    None = 1,
    Inch = 2,
    Centimeter = 3,
}
}

tags! {
pub enum SampleFormat(u16) unknown("An unknown extension sample format") {
    Uint = 1,
    Int = 2,
    IEEEFP = 3,
    Void = 4,
}
}
