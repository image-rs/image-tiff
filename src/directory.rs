use core::fmt;
use std::{collections::BTreeMap, num::NonZeroU64};

use crate::{
    decoder::ifd::Entry,
    tags::{IfdPointer, Tag},
};

/// An Image File Directory (IFD).
#[doc(alias = "IFD")]
pub struct Directory {
    /// There are at most `u16::MAX` entries in any single directory, the count is stored as a
    /// 2-byte value. The order in the file is implied to be ascending by tag value (the decoder
    /// does not mind unordered entries).
    pub(crate) entries: BTreeMap<u16, Entry>,
    pub(crate) next_ifd: Option<NonZeroU64>,
}

impl Directory {
    /// Retrieve the value associated with a tag.
    pub fn get(&self, tag: Tag) -> Option<&Entry> {
        self.entries.get(&tag.to_u16())
    }

    /// Check if the directory contains a specified tag.
    pub fn contains(&self, tag: Tag) -> bool {
        self.entries.contains_key(&tag.to_u16())
    }

    /// Iterate over all known and unknown tags in this directory.
    pub fn iter(&self) -> impl Iterator<Item = (Tag, &Entry)> + '_ {
        self.entries
            .iter()
            .map(|(k, v)| (Tag::from_u16_exhaustive(*k), v))
    }

    /// Insert additional entries into the directory.
    pub fn extend(&mut self, iter: impl IntoIterator<Item = (Tag, Entry)>) {
        // Code size conscious extension, avoid monomorphic extensions with the assumption of these
        // not being performance sensitive in practice. (Maybe we have a polymorphic interface for
        // the crate usage in the future.
        self.extend_inner(iter.into_iter().by_ref())
    }

    /// Get the pointer to the next IFD, if it was defined.
    pub fn next(&self) -> Option<IfdPointer> {
        self.next_ifd.map(|n| IfdPointer(n.get()))
    }

    fn extend_inner(&mut self, iter: &mut dyn Iterator<Item = (Tag, Entry)>) {
        self.entries
            .extend(iter.map(|(ty, entry)| (ty.to_u16(), entry)));
    }
}

impl fmt::Debug for Directory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Directory")
            .field(
                "entries",
                &self.entries.iter().map(|(k, v)| (Tag::from_u16(*k), v)),
            )
            .field("next_ifd", &self.next_ifd)
            .finish()
    }
}
