/// A filter of tags copied in
/// [`DirectoryEncoder::write_metadata_from`][`super::DirectoryEncoder::write_metadata_from`].
pub struct TagFilter {
    unknown: bool,
}

impl TagFilter {
    /// Return a filter that allows all tags from metadata subdirectories.
    pub fn allow_all_in_subdirectories() -> &'static TagFilter {
        static INSTANCE: TagFilter = TagFilter { unknown: true };
        &INSTANCE
    }

    /// Return a filter that has an explicit allow list of tags, for each of a known list of
    /// metadata subdirectories.
    pub fn allow_known() -> &'static TagFilter {
        static INSTANCE: TagFilter = TagFilter { unknown: false };
        &INSTANCE
    }

    /// As per EXIF V3.0 tags, Primary Image (0th IFD).
    pub(crate) fn filter_primary_exif(&self, tag: u16) -> Level {
        match tag {
            // width, height, etc
            0x0100..=0x0103 => Level::SampleLayout,
            // photometric interpretation
            0x0106 => Level::SampleLayout,
            0x010e..=0x0110 => Level::Basic,
            // strip offsets
            0x0111 => Level::SampleLayout,
            0x0112 => Level::Basic,
            // strip byte counts, samples
            0x0115..=0x0117 => Level::SampleLayout,
            0x011a..=0x011b => Level::Basic,
            // planar configuration
            0x011c => Level::Photometric(Photometric::Planar),
            0x0128 => Level::Basic,
            // transfer function
            0x012d => Level::Photometric(Photometric::All),
            0x0131..=0x0132 => Level::Basic,
            0x013b => Level::Basic,
            // Whitepoint, Chromaticities
            0x013e..=0x013f => Level::Photometric(Photometric::All),
            // The table lists these as Not allowed for anything but we may have a choice here for
            // older models of images.
            0x0201..=0x0202 => Level::Special(Special::JpegThumbnail),
            0x0211..=0x0213 => Level::Photometric(Photometric::YCbCr),
            0x0214 => Level::Photometric(Photometric::All),
            0x8298 => Level::Basic,
            0x8769 => Level::Ifd(SubifdKind::ExifPrivate),
            _ => Level::Unknown,
        }
    }

    pub(crate) fn filter_primary_gps(&self, tag: u16) -> Level {
        match tag {
            0x8825 => Level::Ifd(SubifdKind::Gps),
            _ => Level::Unknown,
        }
    }

    pub(crate) fn filter_icc(&self, tag: u16) -> Level {
        match tag {
            0x8ff3 => Level::Basic,
            _ => Level::Unknown,
        }
    }

    pub(crate) fn filter_exif_private(&self, tag: u16) -> Level {
        match tag {
            0x829a => Level::Basic,
            0x829d => Level::Basic,
            0x8822 => Level::Basic,
            0x8824 => Level::Basic,
            0x8827 => Level::Basic,
            0x8828 => Level::Basic,
            0x8830..=0x8835 => Level::Basic,
            0x9000 => Level::Basic,
            0x9003..=0x9004 => Level::Basic,
            0x9010..=0x9012 => Level::Basic,
            // Not allowed for uncompressed data!
            0x9101..=0x9102 => Level::Special(Special::CompressedExclusive),
            0x9201..=0x9209 => Level::Basic,
            0x920a => Level::Basic,
            0x9214 => Level::Basic,
            0x927c => Level::Basic,
            0x9286 => Level::Basic,
            0x9290..=0x9292 => Level::Basic,
            0x9400..=0x9405 => Level::Basic,
            0xa000 => Level::Basic,
            0xa001 => Level::Photometric(Photometric::All),
            0xa002..=0xa003 => Level::Special(Special::CompressedExclusive),
            0xa004 => Level::Basic,
            0xa005 => Level::Ifd(SubifdKind::Interoperability),
            0xa20b..=0xa20c => Level::Basic,
            0xa20e..=0xa210 => Level::Basic,
            0xa214..=0xa215 => Level::Basic,
            0xa217 => Level::Basic,
            0xa300..=0xa302 => Level::Basic,
            0xa401..=0xa40c => Level::Basic,
            0xa420 => Level::Basic,
            0xa430..=0xa43c => Level::Basic,
            0xa460..=0xa462 => Level::Basic,
            0xa500 => Level::Photometric(Photometric::All),
            _ => Level::Unknown,
        }
    }

    fn filter_gps_private(&self, tag: u16) -> Level {
        match tag {
            0x0000..=0x001f => Level::Basic,
            _ => Level::Unknown,
        }
    }

    fn filter_interoperability(&self, tag: u16) -> Level {
        match tag {
            0x0001 => Level::Special(Special::CompressedExclusive),
            _ => Level::Unknown,
        }
    }

    pub(crate) fn filter_secondary_exif(&self, tag: u16) -> Level {
        // No difference for now. The only tabular difference I could spot is the treatment of
        // sample positioning which is mandatory in the primary IFD of YCbCr but optional for all
        // following IFDs..
        self.filter_primary_exif(tag)
    }

    pub(crate) fn filter_secondary_gps(&self, tag: u16) -> Level {
        self.filter_primary_gps(tag)
    }
}

/// The level of support for a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    Unknown,
    /// Value describes the image data layout, which we will not want to write into the encoder's
    /// directory where the image data is written later.
    SampleLayout,
    /// Value is an IFD and must be copied by making a subdirectory.
    Ifd(SubifdKind),
    Photometric(Photometric),
    Basic,
    /// We recognize the tag but it can not be copied by itself through normal means.
    Special(Special),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Photometric {
    All,
    /// Do not copy for non-planar data.
    Planar,
    /// Do not copy for non-YCbCr data.
    YCbCr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubifdKind {
    ExifPrivate,
    Gps,
    Interoperability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Special {
    JpegThumbnail,
    /// May only occur in compressed (JPEG APP segments) data.
    CompressedExclusive,
}
