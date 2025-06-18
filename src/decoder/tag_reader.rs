use crate::{
    TiffError, TiffFormatError, TiffKind, TiffResult,
    decoder::{Limits, decoded_entry::DecodedEntry, stream::EndianReader},
    ifd::{Directory, Value},
    tags::Tag,
};
use std::io::{Read, Seek};

pub(crate) struct TagReader<'a, R: Read + Seek, K: TiffKind> {
    pub reader: &'a mut EndianReader<R>,
    pub ifd: &'a Directory<DecodedEntry<K>>,
    pub limits: &'a Limits,
}

impl<'a, R: Read + Seek, K: TiffKind> TagReader<'a, R, K> {
    pub(crate) fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<Value>> {
        Ok(match self.ifd.get(&tag) {
            Some(entry) => Some(entry.clone().val(self.limits, self.reader)?),
            None => None,
        })
    }

    pub(crate) fn require_tag(&mut self, tag: Tag) -> TiffResult<Value> {
        match self.find_tag(tag)? {
            Some(val) => Ok(val),
            None => Err(TiffError::FormatError(
                TiffFormatError::RequiredTagNotFound(tag),
            )),
        }
    }

    pub fn find_tag_uint_vec<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<Option<Vec<T>>> {
        self.find_tag(tag)?
            .map(|v| v.into_u64_vec())
            .transpose()?
            .map(|v| {
                v.into_iter()
                    .map(|u| {
                        T::try_from(u).map_err(|_| TiffFormatError::InvalidTagValueType(tag).into())
                    })
                    .collect()
            })
            .transpose()
    }
}
