use crate::tags::Tag;
use crate::{TiffError, TiffFormatError, TiffResult};

use super::{Directory};
use crate::decoder::{ifd::Value, Limits, stream::SmartReader};

use futures::{AsyncRead, AsyncSeek};

pub struct AsyncTagReader<'a, R: AsyncRead + AsyncSeek + Unpin + Send> {
    pub reader: &'a mut SmartReader<R>,
    pub ifd: &'a Directory,
    pub limits: &'a Limits,
    pub bigtiff: bool,
}
impl<'a, R: AsyncRead + AsyncSeek + Unpin + Send> AsyncTagReader<'a, R> {
    pub(crate) async fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<Value>> {
        Ok(match self.ifd.get(&tag) {
            Some(entry) => Some(
                entry
                    .clone()
                    .async_val(self.limits, self.bigtiff, self.reader)
                    .await?,
            ),
            None => None,
        }) // dinsdagmiddag   14-16 2e verdieping
    }
    pub(crate) async fn require_tag(&mut self, tag: Tag) -> TiffResult<Value> {
        match self.find_tag(tag).await? {
            Some(val) => Ok(val),
            None => Err(TiffError::FormatError(
                TiffFormatError::RequiredTagNotFound(tag),
            )),
        }
    }
    pub(crate) async fn find_tag_uint_vec<T: TryFrom<u64>>(
        &mut self,
        tag: Tag,
    ) -> TiffResult<Option<Vec<T>>> {
        self.find_tag(tag)
            .await?
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
