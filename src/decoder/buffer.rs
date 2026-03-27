use half::f16;
use std::alloc::{Layout, LayoutError};

use crate::{
    bytecast,
    decoder::{BufferLayoutPreference, Limits},
    error::{TiffError, TiffResult, TiffUnsupportedError},
};

/// Result of a decoding process
#[derive(Debug)]
pub enum DecodingResult {
    /// A vector of unsigned bytes
    U8(Vec<u8>),
    /// A vector of unsigned words
    U16(Vec<u16>),
    /// A vector of 32 bit unsigned ints
    U32(Vec<u32>),
    /// A vector of 64 bit unsigned ints
    U64(Vec<u64>),
    /// A vector of 16 bit IEEE floats (held in u16)
    F16(Vec<f16>),
    /// A vector of 32 bit IEEE floats
    F32(Vec<f32>),
    /// A vector of 64 bit IEEE floats
    F64(Vec<f64>),
    /// A vector of 8 bit signed ints
    I8(Vec<i8>),
    /// A vector of 16 bit signed ints
    I16(Vec<i16>),
    /// A vector of 32 bit signed ints
    I32(Vec<i32>),
    /// A vector of 64 bit signed ints
    I64(Vec<i64>),
}

impl DecodingResult {
    /// Reallocate the buffer to decode all planes of the indicated layout.
    pub fn resize_to(
        &mut self,
        buffer: &BufferLayoutPreference,
        limits: &Limits,
    ) -> Result<(), TiffError> {
        let sample_type = buffer.sample_type.ok_or(TiffError::UnsupportedError(
            TiffUnsupportedError::UnknownInterpretation,
        ))?;

        let extent = sample_type.extent_for_bytes(buffer.complete_len);
        self.resize_to_extent(extent, limits)
    }

    pub(crate) fn resize_to_extent(
        &mut self,
        extent: DecodingExtent,
        limits: &Limits,
    ) -> Result<(), TiffError> {
        // FIXME: we *can* reuse the allocation sometimes.
        *self = extent.to_result_buffer(limits)?;
        Ok(())
    }

    fn new<T: Default + Copy>(
        size: usize,
        limits: &Limits,
        from_fn: fn(Vec<T>) -> Self,
    ) -> TiffResult<DecodingResult> {
        if size > limits.decoding_buffer_size / core::mem::size_of::<T>() {
            Err(TiffError::LimitsExceeded)
        } else {
            Ok(from_fn(vec![T::default(); size]))
        }
    }

    fn new_u8(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U8)
    }

    fn new_u16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U16)
    }

    fn new_u32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U32)
    }

    fn new_u64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::U64)
    }

    fn new_f32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::F32)
    }

    fn new_f64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::F64)
    }

    fn new_f16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::F16)
    }

    fn new_i8(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I8)
    }

    fn new_i16(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I16)
    }

    fn new_i32(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I32)
    }

    fn new_i64(size: usize, limits: &Limits) -> TiffResult<DecodingResult> {
        Self::new(size, limits, DecodingResult::I64)
    }

    /// Get a view of this buffer starting from the nth _sample_ of the current type.
    pub fn as_buffer(&mut self, start: usize) -> DecodingBuffer<'_> {
        match *self {
            DecodingResult::U8(ref mut buf) => DecodingBuffer::U8(&mut buf[start..]),
            DecodingResult::U16(ref mut buf) => DecodingBuffer::U16(&mut buf[start..]),
            DecodingResult::U32(ref mut buf) => DecodingBuffer::U32(&mut buf[start..]),
            DecodingResult::U64(ref mut buf) => DecodingBuffer::U64(&mut buf[start..]),
            DecodingResult::F16(ref mut buf) => DecodingBuffer::F16(&mut buf[start..]),
            DecodingResult::F32(ref mut buf) => DecodingBuffer::F32(&mut buf[start..]),
            DecodingResult::F64(ref mut buf) => DecodingBuffer::F64(&mut buf[start..]),
            DecodingResult::I8(ref mut buf) => DecodingBuffer::I8(&mut buf[start..]),
            DecodingResult::I16(ref mut buf) => DecodingBuffer::I16(&mut buf[start..]),
            DecodingResult::I32(ref mut buf) => DecodingBuffer::I32(&mut buf[start..]),
            DecodingResult::I64(ref mut buf) => DecodingBuffer::I64(&mut buf[start..]),
        }
    }
}

// A buffer for image decoding
pub enum DecodingBuffer<'a> {
    /// A slice of unsigned bytes
    U8(&'a mut [u8]),
    /// A slice of unsigned words
    U16(&'a mut [u16]),
    /// A slice of 32 bit unsigned ints
    U32(&'a mut [u32]),
    /// A slice of 64 bit unsigned ints
    U64(&'a mut [u64]),
    /// A slice of 16 bit IEEE floats
    F16(&'a mut [f16]),
    /// A slice of 32 bit IEEE floats
    F32(&'a mut [f32]),
    /// A slice of 64 bit IEEE floats
    F64(&'a mut [f64]),
    /// A slice of 8 bits signed ints
    I8(&'a mut [i8]),
    /// A slice of 16 bits signed ints
    I16(&'a mut [i16]),
    /// A slice of 32 bits signed ints
    I32(&'a mut [i32]),
    /// A slice of 64 bits signed ints
    I64(&'a mut [i64]),
}

impl<'a> DecodingBuffer<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            DecodingBuffer::U8(buf) => buf,
            DecodingBuffer::I8(buf) => bytecast::i8_as_ne_bytes(buf),
            DecodingBuffer::U16(buf) => bytecast::u16_as_ne_bytes(buf),
            DecodingBuffer::I16(buf) => bytecast::i16_as_ne_bytes(buf),
            DecodingBuffer::U32(buf) => bytecast::u32_as_ne_bytes(buf),
            DecodingBuffer::I32(buf) => bytecast::i32_as_ne_bytes(buf),
            DecodingBuffer::U64(buf) => bytecast::u64_as_ne_bytes(buf),
            DecodingBuffer::I64(buf) => bytecast::i64_as_ne_bytes(buf),
            DecodingBuffer::F16(buf) => bytecast::f16_as_ne_bytes(buf),
            DecodingBuffer::F32(buf) => bytecast::f32_as_ne_bytes(buf),
            DecodingBuffer::F64(buf) => bytecast::f64_as_ne_bytes(buf),
        }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        match self {
            DecodingBuffer::U8(buf) => buf,
            DecodingBuffer::I8(buf) => bytecast::i8_as_ne_mut_bytes(buf),
            DecodingBuffer::U16(buf) => bytecast::u16_as_ne_mut_bytes(buf),
            DecodingBuffer::I16(buf) => bytecast::i16_as_ne_mut_bytes(buf),
            DecodingBuffer::U32(buf) => bytecast::u32_as_ne_mut_bytes(buf),
            DecodingBuffer::I32(buf) => bytecast::i32_as_ne_mut_bytes(buf),
            DecodingBuffer::U64(buf) => bytecast::u64_as_ne_mut_bytes(buf),
            DecodingBuffer::I64(buf) => bytecast::i64_as_ne_mut_bytes(buf),
            DecodingBuffer::F16(buf) => bytecast::f16_as_ne_mut_bytes(buf),
            DecodingBuffer::F32(buf) => bytecast::f32_as_ne_mut_bytes(buf),
            DecodingBuffer::F64(buf) => bytecast::f64_as_ne_mut_bytes(buf),
        }
    }

    pub fn byte_len(&self) -> usize {
        self.as_bytes().len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodingSampleType {
    U8,
    U16,
    U32,
    U64,
    F16,
    F32,
    F64,
    I8,
    I16,
    I32,
    I64,
}

impl DecodingSampleType {
    pub(crate) fn extent_for_bytes(self, bytes: usize) -> DecodingExtent {
        match self {
            DecodingSampleType::U8 => DecodingExtent::U8(bytes),
            DecodingSampleType::U16 => DecodingExtent::U16(bytes.div_ceil(2)),
            DecodingSampleType::U32 => DecodingExtent::U32(bytes.div_ceil(4)),
            DecodingSampleType::U64 => DecodingExtent::U64(bytes.div_ceil(8)),
            DecodingSampleType::I8 => DecodingExtent::I8(bytes),
            DecodingSampleType::I16 => DecodingExtent::I16(bytes.div_ceil(2)),
            DecodingSampleType::I32 => DecodingExtent::I32(bytes.div_ceil(4)),
            DecodingSampleType::I64 => DecodingExtent::I64(bytes.div_ceil(8)),
            DecodingSampleType::F16 => DecodingExtent::F16(bytes.div_ceil(2)),
            DecodingSampleType::F32 => DecodingExtent::F32(bytes.div_ceil(4)),
            DecodingSampleType::F64 => DecodingExtent::F64(bytes.div_ceil(8)),
        }
    }
}

/// The count and matching discriminant for a `DecodingBuffer`.
#[derive(Clone)]
pub(crate) enum DecodingExtent {
    U8(usize),
    U16(usize),
    U32(usize),
    U64(usize),
    F16(usize),
    F32(usize),
    F64(usize),
    I8(usize),
    I16(usize),
    I32(usize),
    I64(usize),
}

impl DecodingExtent {
    pub(crate) fn to_result_buffer(&self, limits: &Limits) -> TiffResult<DecodingResult> {
        match *self {
            DecodingExtent::U8(count) => DecodingResult::new_u8(count, limits),
            DecodingExtent::U16(count) => DecodingResult::new_u16(count, limits),
            DecodingExtent::U32(count) => DecodingResult::new_u32(count, limits),
            DecodingExtent::U64(count) => DecodingResult::new_u64(count, limits),
            DecodingExtent::F16(count) => DecodingResult::new_f16(count, limits),
            DecodingExtent::F32(count) => DecodingResult::new_f32(count, limits),
            DecodingExtent::F64(count) => DecodingResult::new_f64(count, limits),
            DecodingExtent::I8(count) => DecodingResult::new_i8(count, limits),
            DecodingExtent::I16(count) => DecodingResult::new_i16(count, limits),
            DecodingExtent::I32(count) => DecodingResult::new_i32(count, limits),
            DecodingExtent::I64(count) => DecodingResult::new_i64(count, limits),
        }
    }

    pub(crate) fn preferred_layout(self) -> TiffResult<Layout> {
        fn overflow(_: LayoutError) -> TiffError {
            TiffError::LimitsExceeded
        }

        match self {
            DecodingExtent::U8(count) => Layout::array::<u8>(count),
            DecodingExtent::U16(count) => Layout::array::<u16>(count),
            DecodingExtent::U32(count) => Layout::array::<u32>(count),
            DecodingExtent::U64(count) => Layout::array::<u64>(count),
            DecodingExtent::F16(count) => Layout::array::<f16>(count),
            DecodingExtent::F32(count) => Layout::array::<f32>(count),
            DecodingExtent::F64(count) => Layout::array::<f64>(count),
            DecodingExtent::I8(count) => Layout::array::<i8>(count),
            DecodingExtent::I16(count) => Layout::array::<i16>(count),
            DecodingExtent::I32(count) => Layout::array::<i32>(count),
            DecodingExtent::I64(count) => Layout::array::<i64>(count),
        }
        .map_err(overflow)
    }

    pub(crate) fn sample_type(&self) -> DecodingSampleType {
        match *self {
            DecodingExtent::U8(_) => DecodingSampleType::U8,
            DecodingExtent::U16(_) => DecodingSampleType::U16,
            DecodingExtent::U32(_) => DecodingSampleType::U32,
            DecodingExtent::U64(_) => DecodingSampleType::U64,
            DecodingExtent::F16(_) => DecodingSampleType::F16,
            DecodingExtent::F32(_) => DecodingSampleType::F32,
            DecodingExtent::F64(_) => DecodingSampleType::F64,
            DecodingExtent::I8(_) => DecodingSampleType::I8,
            DecodingExtent::I16(_) => DecodingSampleType::I16,
            DecodingExtent::I32(_) => DecodingSampleType::I32,
            DecodingExtent::I64(_) => DecodingSampleType::I64,
        }
    }
}
