// Special thanks to Alice for the help: https://users.rust-lang.org/t/63019/6
use crate::decoder::stream::ByteOrder;
use std::io::{Result, SeekFrom};
use std::pin::Pin;

use futures::{
    future::BoxFuture,
    io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, BufReader},
    Future,
};

// pub struct RangedStreamer {
//     pos: u64,
//     length: u64, // total size
//     state: State,
//     range_get: F,
//     min_request_size: usize, // requests have at least this size
// }

// enum State {
//     HasChunk(SeekOutput),
//     Seeking(BoxFuture<'static, std::io::Result<SeekOutput>>),
// }

// pub struct SeekOutput {
//     pub start: u64,
//     pub data: Vec<u8>,
// }

// pub type F = std::sync::Arc<
//     dyn Fn(u64, usize) -> BoxFuture<'static, std::io::Result<SeekOutput>> + Send + Sync,
// >;

// impl RangedStreamer {
//     pub fn new(length: usize, min_request_size: usize, range_get: F) -> Self {
//         let length = length as u64;
//         Self {
//             pos: 0,
//             length,
//             state: State::HasChunk(SeekOutput {
//                 start: 0,
//                 data: vec![],
//             }),
//             range_get,
//             min_request_size,
//         }
//     }
// }

// // whether `test_interval` is inside `a` (start, length).
// async fn range_includes(a: (usize, usize), test_interval: (usize, usize)) -> bool {
//     if test_interval.0 < a.0 {
//         return false;
//     }
//     let test_end = test_interval.0 + test_interval.1;
//     let a_end = a.0 + a.1;
//     if test_end > a_end {
//         return false;
//     }
//     true
// }

// impl AsyncRead for RangedStreamer {
//     fn poll_read(
//         mut self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//         buf: &mut [u8],
//     ) -> std::task::Poll<Result<usize>> {
//         let requested_range = (self.pos as usize, buf.len());
//         let min_request_size = self.min_request_size;
//         match &mut self.state {
//             State::HasChunk(output) => {
//                 let existing_range = (output.start as usize, output.data.len());
//                 if range_includes(existing_range, requested_range) {
//                     let offset = requested_range.0 - existing_range.0;
//                     buf.copy_from_slice(&output.data[offset..offset + buf.len()]);
//                     self.pos += buf.len() as u64;
//                     std::task::Poll::Ready(Ok(buf.len()))
//                 } else {
//                     let start = requested_range.0 as u64;
//                     let length = std::cmp::max(min_request_size, requested_range.1);
//                     let future = (self.range_get)(start, length);
//                     self.state = State::Seeking(Box::pin(future));
//                     self.poll_read(cx, buf)
//                 }
//             }
//             State::Seeking(ref mut future) => match Pin::new(future).poll(cx) {
//                 std::task::Poll::Ready(v) => {
//                     match v {
//                         Ok(output) => self.state = State::HasChunk(output),
//                         Err(e) => return std::task::Poll::Ready(Err(e)),
//                     };
//                     self.poll_read(cx, buf)
//                 }
//                 std::task::Poll::Pending => std::task::Poll::Pending,
//             },
//         }
//     }
// }

// impl AsyncSeek for RangedStreamer {
//     fn poll_seek(
//         mut self: std::pin::Pin<&mut Self>,
//         _: &mut std::task::Context<'_>,
//         pos: std::io::SeekFrom,
//     ) -> std::task::Poll<Result<u64>> {
//         match pos {
//             SeekFrom::Start(pos) => self.pos = pos,
//             SeekFrom::End(pos) => self.pos = (self.length as i64 + pos) as u64,
//             SeekFrom::Current(pos) => self.pos = (self.pos as i64 + pos) as u64,
//         };
//         std::task::Poll::Ready(Ok(self.pos))
//     }
// }

// pub type DeflateReader<R> = flate2::read::ZlibDecoder<R>;

// ///
// /// ## LZW Reader
// ///

// /// Reader that decompresses LZW streams
// pub struct LZWReader<R: AsyncRead> {
//     reader: BufReader<Take<R>>,
//     decoder: weezl::decode::Decoder,
// }

// impl<R: Read> LZWReader<R> {
//     /// Wraps a reader
//     pub fn new(reader: R, compressed_length: usize) -> LZWReader<R> {
//         Self {
//             reader: BufReader::with_capacity(
//                 (32 * 1024).min(compressed_length),
//                 reader.take(u64::try_from(compressed_length).unwrap()),
//             ),
//             decoder: weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8),
//         }
//     }
// }

// impl<R: Read> Read for LZWReader<R> {
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         loop {
//             let result = self.decoder.decode_bytes(self.reader.fill_buf()?, buf);
//             self.reader.consume(result.consumed_in);

//             match result.status {
//                 Ok(weezl::LzwStatus::Ok) => {
//                     if result.consumed_out == 0 {
//                         continue;
//                     } else {
//                         return Ok(result.consumed_out);
//                     }
//                 }
//                 Ok(weezl::LzwStatus::NoProgress) => {
//                     assert_eq!(result.consumed_in, 0);
//                     assert_eq!(result.consumed_out, 0);
//                     assert!(self.reader.buffer().is_empty());
//                     return Err(io::Error::new(
//                         io::ErrorKind::UnexpectedEof,
//                         "no lzw end code found",
//                     ));
//                 }
//                 Ok(weezl::LzwStatus::Done) => {
//                     return Ok(result.consumed_out);
//                 }
//                 Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidData, err)),
//             }
//         }
//     }
// }

pub struct AsyncSmartReader<R: AsyncRead + AsyncSeek + Unpin> {
    reader: R,
    pub byte_order: ByteOrder,
}

impl<R: AsyncRead + AsyncSeek + Unpin> AsyncSmartReader<R> {
    pub async fn goto_offset(&mut self, offset: u64) -> Result<()> {
        self.reader.seek(SeekFrom::Start(offset)).await.map(|_| ())
    }

    pub fn wrap(reader: R, byte_order: ByteOrder) -> Self {
        AsyncSmartReader { reader, byte_order }
    }
}

impl<R: AsyncRead + AsyncSeek + Unpin> AsyncRead for AsyncSmartReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<Result<usize>> {
        let pinned = std::pin::Pin::new(&mut self.get_mut().reader);
        pinned.poll_read(cx, buf)
    }
}

impl<R: AsyncRead + AsyncSeek + Unpin> AsyncSeek for AsyncSmartReader<R> {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        pos: SeekFrom,
    ) -> std::task::Poll<Result<u64>> {
        let pinned = std::pin::Pin::new(&mut self.get_mut().reader);
        pinned.poll_seek(cx, pos)
    }
}

impl<R: AsyncRead + AsyncSeek + Unpin> AsyncSmartReader<R> {
    /// Byte order that should be adhered to
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    /// Reads an u16
    #[inline(always)]
    pub async fn read_u16(&mut self) -> Result<u16> {
        let mut n = [0u8; 2];
        self.read_exact(&mut n).await?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => u16::from_le_bytes(n),
            ByteOrder::BigEndian => u16::from_be_bytes(n),
        })
    }

    /// Reads an i8
    #[inline(always)]
    pub async fn read_i8(&mut self) -> Result<i8> {
        let mut n = [0u8; 1];
        self.read_exact(&mut n).await?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i8::from_le_bytes(n),
            ByteOrder::BigEndian => i8::from_be_bytes(n),
        })
    }

    /// Reads an i16
    #[inline(always)]
    pub async fn read_i16(&mut self) -> Result<i16> {
        let mut n = [0u8; 2];
        self.read_exact(&mut n).await?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i16::from_le_bytes(n),
            ByteOrder::BigEndian => i16::from_be_bytes(n),
        })
    }

    /// Reads an u32
    #[inline(always)]
    pub async fn read_u32(&mut self) -> Result<u32> {
        let mut n = [0u8; 4];
        self.read_exact(&mut n).await?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        })
    }

    /// Reads an i32
    #[inline(always)]
    pub async fn read_i32(&mut self) -> Result<i32> {
        let mut n = [0u8; 4];
        self.read_exact(&mut n).await?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i32::from_le_bytes(n),
            ByteOrder::BigEndian => i32::from_be_bytes(n),
        })
    }

    /// Reads an u64
    #[inline(always)]
    pub async fn read_u64(&mut self) -> Result<u64> {
        let mut n = [0u8; 8];
        self.read_exact(&mut n).await?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        })
    }

    /// Reads an i64
    #[inline(always)]
    pub async fn read_i64(&mut self) -> Result<i64> {
        let mut n = [0u8; 8];
        self.read_exact(&mut n).await?;
        Ok(match self.byte_order() {
            ByteOrder::LittleEndian => i64::from_le_bytes(n),
            ByteOrder::BigEndian => i64::from_be_bytes(n),
        })
    }

    /// Reads an f32
    #[inline(always)]
    pub async fn read_f32(&mut self) -> Result<f32> {
        let mut n = [0u8; 4];
        self.read_exact(&mut n).await?;
        Ok(f32::from_bits(match self.byte_order() {
            ByteOrder::LittleEndian => u32::from_le_bytes(n),
            ByteOrder::BigEndian => u32::from_be_bytes(n),
        }))
    }

    /// Reads an f64
    #[inline(always)]
    pub async fn read_f64(&mut self) -> Result<f64> {
        let mut n = [0u8; 8];
        self.read_exact(&mut n).await?;
        Ok(f64::from_bits(match self.byte_order() {
            ByteOrder::LittleEndian => u64::from_le_bytes(n),
            ByteOrder::BigEndian => u64::from_be_bytes(n),
        }))
    }
}
// /// Reader that is aware of the byte order.
// #[derive(Debug)]
// pub struct AsyncSmartReader<R>
// where
//     R: AsyncRead,
// {
//     reader: R,
//     pub byte_order: ByteOrder,
// }

// impl<R> AsyncSmartReader<R>
// where
//     R: AsyncRead,
// {
//     /// Wraps a reader
//     pub fn wrap(reader: R, byte_order: ByteOrder) -> AsyncSmartReader<R> {
//         AsyncSmartReader { reader, byte_order }
//     }
//     pub fn into_inner(self) -> R {
//         self.reader
//     }
// }
// impl<R: AsyncRead + AsyncSeek> AsyncSmartReader<R> {

//     #[inline(always)]
//     fn byte_order(&self) -> ByteOrder {
//         self.byte_order
//     }
// }

// impl<R: AsyncRead> AsyncSeek for AsyncSmartReader<R> {
//     fn poll_seek(
//                 self: Pin<&mut Self>,
//                 cx: &mut std::task::Context<'_>,
//                 pos: SeekFrom,
//             ) -> std::task::Poll<Result<u64>> {

//     }
// }
