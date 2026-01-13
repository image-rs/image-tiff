use crate::tags::{PhotometricInterpretation, PlanarConfiguration};

/// A filter of tags copied in
/// [`DirectoryEncoder::write_metadata_from`][`super::DirectoryEncoder::write_metadata_from`].
pub struct TagFilter {
    filter_level: FilterLevel,
}

impl TagFilter {
    /// Return a filter that allows all tags from metadata subdirectories.
    pub fn allow_all_in_subdirectories() -> &'static TagFilter {
        static INSTANCE: TagFilter = TagFilter {
            filter_level: FilterLevel::All,
        };
        &INSTANCE
    }

    /// Return a filter that has an explicit allow list of tags, for each of a known list of
    /// metadata subdirectories.
    pub fn only_known_tags() -> &'static TagFilter {
        static INSTANCE: TagFilter = TagFilter {
            filter_level: FilterLevel::Known,
        };
        &INSTANCE
    }

    /// FIXME: take a configuration by which to filter out complete metadata kinds.
    /// FIXME: when we iterate over a whole directory of tags, this iteration is more expensive
    /// than necessary. Tags within a directory should be ordered. We can thus perform a binary set
    /// join which is more efficient than checking the individual filter on each. Does not matter
    /// much in most cases though.
    pub(crate) fn filter_primary(&self, tag: u16) -> Level {
        let as_exif = self.filter_primary_exif(tag);

        if !matches!(as_exif, Level::Unknown) {
            return as_exif;
        }

        let as_gps = self.filter_primary_gps(tag);

        if !matches!(as_gps, Level::Unknown) {
            return as_gps;
        }

        let as_icc = self.filter_icc(tag);

        if !matches!(as_icc, Level::Unknown) {
            return as_icc;
        }

        Level::Unknown
    }

    /// As per EXIF V3.0 tags, Primary Image (0th IFD).
    pub(crate) fn filter_primary_exif(&self, tag: u16) -> Level {
        match tag {
            // width, height, etc
            0x0100..=0x0103 => Level::SampleLayout,
            // photometric interpretation
            0x0106 => Level::SampleLayout,
            // data tied to the camera and customized description, potentially PI.
            0x010f..=0x0110 => Level::Value,
            // strip offsets
            0x0111 => Level::SampleLayout,
            // orientation
            0x0112 => Level::Value,
            // strip byte counts, samples
            0x0115..=0x0117 => Level::SampleLayout,
            // pixel dimensions (ratio). Should be generic enough.
            0x011a..=0x011b => Level::Value,
            // planar configuration
            0x011c => Level::Photometric(Photometric::Planar),
            // units for 0x011a,0x011b
            0x0128 => Level::Value,
            // transfer function
            0x012d => Level::Photometric(Photometric::All),
            // file data time, software pipeline. I'm wary of privacy implications.
            0x0131..=0x0132 => Level::Value,
            // artist, clearly PI unless you explicitly wish it to be attributed.
            0x013b => Level::Value,
            // Whitepoint, Chromaticities
            0x013e..=0x013f => Level::Photometric(Photometric::All),
            // The table lists these as Not allowed for anything but we may have a choice here for
            // older models of images.
            0x0201..=0x0202 => Level::Special(Special::JpegThumbnail),
            0x0211..=0x0213 => Level::Photometric(Photometric::YCbCr),
            0x0214 => Level::Photometric(Photometric::All),
            // Copyright holder (not same as artist).
            0x8298 => Level::Value,
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
            0x8ff3 => Level::Value,
            _ => Level::Unknown,
        }
    }

    pub(crate) fn filter_exif_private(&self, tag: u16) -> Level {
        match tag {
            // Maybe this is not personal identifying but some equipment may be very specifically
            // known through these. And it's not critical to displaying it correctly.
            0x829a => Level::Value,
            0x829d => Level::Value,
            0x8822 => Level::Value,
            0x8824 => Level::Value,
            0x8827 => Level::Value,
            // Opto-electric coefficients
            0x8828 => Level::Value,
            0x8830..=0x8835 => Level::Value,
            0x9000 => Level::Value,
            0x9003..=0x9004 => Level::Value,
            0x9010..=0x9012 => Level::Value,
            // Not allowed for uncompressed data!
            0x9101..=0x9102 => Level::Special(Special::CompressedExclusive),
            0x9201..=0x9209 => Level::Value,
            0x920a => Level::Value,
            0x9214 => Level::Value,
            0x927c => Level::Value,
            0x9286 => Level::Value,
            0x9290..=0x9292 => Level::Value,
            0x9400..=0x9405 => Level::Value,
            0xa000 => Level::Value,
            0xa001 => Level::Photometric(Photometric::All),
            0xa002..=0xa003 => Level::Special(Special::CompressedExclusive),
            0xa004 => Level::Value,
            0xa005 => Level::Ifd(SubifdKind::Interoperability),
            0xa20b..=0xa20c => Level::Value,
            0xa20e..=0xa210 => Level::Value,
            0xa214..=0xa215 => Level::Value,
            0xa217 => Level::Value,
            0xa300..=0xa302 => Level::Value,
            0xa401..=0xa40c => Level::Value,
            0xa420 => Level::Value,
            0xa430..=0xa43c => Level::Value,
            0xa460..=0xa462 => Level::Value,
            0xa500 => Level::Photometric(Photometric::All),
            _ => Level::Unknown,
        }
    }

    pub(crate) fn filter_gps_private(&self, tag: u16) -> Level {
        match tag {
            0x0000..=0x001f => Level::Value,
            _ => Level::Unknown,
        }
    }

    pub(crate) fn should_keep_for_image_writer(
        &self,
        level: Level,
        target: &super::EncodeMetadataApplicability,
    ) -> Choice {
        match level {
            Level::Unknown if matches!(self.filter_level, FilterLevel::All) => Choice::Filtered,
            // Sample Layout is controlled by the writer (dimensions, color type, …). Do not copy!
            Level::SampleLayout => Choice::Discard,
            Level::Ifd(subifd_kind) => Choice::Descend(subifd_kind),
            Level::Photometric(photometric) => {
                match (photometric, target.planar, target.photometric) {
                    (Photometric::All, _, _) => Choice::Ok,
                    (Photometric::Planar, Some(PlanarConfiguration::Planar), _) => Choice::Ok,
                    (Photometric::Planar, None, _)
                        if matches!(self.filter_level, FilterLevel::All) =>
                    {
                        Choice::Ok
                    }
                    (Photometric::YCbCr, _, Some(PhotometricInterpretation::YCbCr)) => Choice::Ok,
                    (Photometric::YCbCr, _, None)
                        if matches!(self.filter_level, FilterLevel::All) =>
                    {
                        Choice::Ok
                    }
                    _ => Choice::Discard,
                }
            }
            Level::Value => Choice::Ok,
            Level::Special(special) => Choice::Special(special),
            _ => Choice::Discard,
        }
    }
}

pub(crate) enum Choice {
    Ok,
    Descend(SubifdKind),
    /// Tag is inapplicable to be copied.
    Discard,
    /// Tag was chosen not to be copied.
    Filtered,
    Special(Special),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterLevel {
    All,
    Known,
}

/// The level of support for a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    Unknown,
    /// Value describes the image data layout, which we will not want to write into the encoder's
    /// directory where the image data is written later.
    ///
    /// These are treated as privacy preserving.
    SampleLayout,
    /// Value is an IFD and must be copied by making a subdirectory.
    ///
    /// These are treated as privacy preserving, independent of the tags within the subdirectory
    /// which may not be. (For instance the presence of an empty directory is fine but we will omit
    /// a GPS anyways).
    Ifd(SubifdKind),
    /// Value is related to photometric interpretation and should not be copied when the target
    /// does not match the expected interpretation.
    ///
    /// These are *generally* treated as privacy preserving.
    Photometric(Photometric),
    /// This is a value we know of.
    /// FIXME: we do not differentiate by personal identifiable information and otherwise which is
    /// quite important for some variations of copying metadata.
    Value,
    /// We recognize the tag but it can not be copied by itself through normal means.
    ///
    /// Privacy treatment depends on the tag.
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
    /// Thumbnail which we regard as PI since we do not deep-clean the JPEG structure.
    JpegThumbnail,
    /// May only occur in compressed (JPEG APP segments) data.
    CompressedExclusive,
}
