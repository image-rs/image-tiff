use std::collections::hash_map::IntoIter;
use std::io::{Read, Seek};

use crate::decoder::ifd::{Directory, Value};
use crate::decoder::stream::SmartReader;
use crate::decoder::{ifd, Limits};
use crate::tags::Tag;
use crate::TiffResult;

pub(crate) struct TagIter<'a, R>
where
    R: Read + Seek,
{
    iter: IntoIter<Tag, ifd::Entry>,
    limits: &'a Limits,
    bigtiff: bool,
    reader: &'a mut SmartReader<R>,
}

impl<'a, R> TagIter<'a, R>
where
    R: Read + Seek,
{
    pub fn new(
        directory: Directory,
        limits: &'a Limits,
        bigtiff: bool,
        reader: &'a mut SmartReader<R>,
    ) -> Self {
        Self {
            iter: directory.into_iter(),
            limits,
            bigtiff,
            reader,
        }
    }
}

impl<'a, R> Iterator for TagIter<'a, R>
where
    R: Read + Seek,
{
    type Item = TiffResult<(Tag, Value)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(tag, entry)| {
            entry
                .val(self.limits, self.bigtiff, self.reader)
                .map(|value| (tag, value))
        })
    }
}
