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
    pub fn empty(next: IfdPointer) -> Self {
        Directory {
            entries: BTreeMap::new(),
            next_ifd: NonZeroU64::new(next.0),
        }
    }

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
    ///
    /// Note that a directory can contain at most `u16::MAX` values. There may be one entry that
    /// does not fit into the directory. This entry is silently ignored (please check [`Self::len`]
    /// to detect the condition). Providing a tag multiple times or a tag that already exists
    /// within this directory overwrites the entry.
    pub fn extend(&mut self, iter: impl IntoIterator<Item = (Tag, Entry)>) {
        // Code size conscious extension, avoid monomorphic extensions with the assumption of these
        // not being performance sensitive in practice. (Maybe we have a polymorphic interface for
        // the crate usage in the future.
        self.extend_inner(iter.into_iter().by_ref())
    }

    /// Get the length as a 2-byte integer.
    ///
    /// This is *almost* naturally bounded by the tag type being a `u16` itself. The
    /// bounds are upheld when extending the iteration, i.e. this always corresponds to the
    /// underlying data.
    pub fn len(&self) -> u16 {
        // The keys are `u16`. Since IFDs are required to have at least one entry this would have
        // been a trivial thing to do in the specification by storing it minus one but alas.
        self.entries.len() as u16
    }

    /// Get the pointer to the next IFD, if it was defined.
    pub fn next(&self) -> Option<IfdPointer> {
        self.next_ifd.map(|n| IfdPointer(n.get()))
    }

    fn extend_inner(&mut self, iter: &mut dyn Iterator<Item = (Tag, Entry)>) {
        for (tag, entry) in iter {
            let is_full = self.entries.len() == u16::MAX as usize;
            // If the tag is already present, it will be overwritten.
            let map_entry = self.entries.entry(tag.to_u16());

            if is_full {
                // Only allowed to modify if the entry is already present.
                map_entry.and_modify(|place| *place = entry);
            } else {
                match map_entry {
                    std::collections::btree_map::Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(entry);
                    }
                    std::collections::btree_map::Entry::Occupied(mut occupied_entry) => {
                        occupied_entry.insert(entry);
                    }
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::{Directory, IfdPointer};
    use crate::{decoder::ifd::Entry, tags::Tag};

    #[test]
    fn directory_upholds_len_invariant() {
        let mut dir = Directory::empty(IfdPointer(0));
        assert_eq!(dir.len(), 0);

        let bogus_entry = Entry::new_u64(crate::tags::Type::BYTE, 0x0, [0; 8]);
        dir.extend((0..=u16::MAX).map(|i| {
            let tag = Tag::Unknown(i);
            let entry = bogus_entry.clone();
            (tag, entry)
        }));

        assert_eq!(dir.len(), u16::MAX);

        // Ensure the other tags are still writable.
        dir.extend([(
            Tag::Unknown(0),
            Entry::new_u64(crate::tags::Type::BYTE, 0x42, [0; 8]),
        )]);

        assert_eq!(
            dir.get(Tag::Unknown(0))
                .expect("tag 0 should be present after overwriting")
                .count(),
            0x42
        );
    }

    #[test]
    fn directory_multiple_entries() {
        let mut dir = Directory::empty(IfdPointer(0));
        assert_eq!(dir.len(), 0);

        dir.extend((0..=u16::MAX).map(|i| {
            let tag = Tag::Unknown(1);
            let entry = Entry::new_u64(crate::tags::Type::BYTE, i.into(), [0; 8]);
            (tag, entry)
        }));

        assert_eq!(dir.len(), 1, "Only one tag was ever modified");

        assert_eq!(
            dir.get(Tag::Unknown(1))
                .expect("tag 1 should be present after this chain")
                .count(),
            u16::MAX.into()
        );
    }

    #[test]
    fn iteration_order() {
        let mut dir = Directory::empty(IfdPointer(0));
        assert_eq!(dir.len(), 0);

        let fake_entry = Entry::new_u64(crate::tags::Type::BYTE, 0, [0; 8]);
        dir.extend((0..32).map(|i| {
            let tag = Tag::Unknown(i);
            let entry = fake_entry.clone();
            (tag, entry)
        }));

        let iter_order: Vec<u16> = dir.iter().map(|(tag, _e)| tag.to_u16()).collect();
        assert_eq!(
            iter_order,
            (0..32).collect::<Vec<_>>(),
            "Tags must be in ascending order according to the specification"
        );
    }
}
