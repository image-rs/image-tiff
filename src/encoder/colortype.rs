use crate::tags::{PhotometricInterpretation, SampleFormat};

/// Apply floating-point predictor encoding to a row of floating-point samples.
///
/// The floating-point predictor works by:
/// 1. Converting each value to big-endian bytes
/// 2. Shuffling bytes so all first bytes are together, all second bytes, etc.
/// 3. Applying byte-level horizontal difference (wrapping_sub) across the entire buffer
///
/// The decoder reverses this by:
/// 1. Applying cumulative sum (wrapping_add) on the entire byte stream
/// 2. De-shuffling bytes back into float values
macro_rules! fp_predict {
    ($name:ident, $float_type:ty) => {
        fn $name(row: &[$float_type], samples: usize, result: &mut Vec<u8>) {
            const BYTE_SIZE: usize = core::mem::size_of::<$float_type>();
            let num_values = row.len();
            let byte_count = num_values * BYTE_SIZE;
            let start_len = result.len();

            // Allocate zeroed space so we can write to arbitrary positions
            result.resize(start_len + byte_count, 0);
            let output = &mut result[start_len..];

            // Step 1 & 2: Convert to big-endian and scatter bytes
            for (i, &value) in row.iter().enumerate() {
                let bytes = value.to_be_bytes();
                for b in 0..BYTE_SIZE {
                    output[b * num_values + i] = bytes[b];
                }
            }

            // Step 3: Apply byte-level horizontal difference across the entire buffer.
            // The prediction spans across quarter boundaries, just like the decoder's cumulative sum.
            //
            // We process elements in reverse in batches of 16 to enable autovectorization.
            // Reverse order ensures that when we compute output[i] - output[i-samples],
            // the value at output[i-samples] hasn't been overwritten yet (critical when
            // samples < 16, which is the common case: 1, 3, 4, 5).
            //
            // 8 bits * 16 = 128 bits, the width of a typical SIMD register.
            // Even if the target has no vector instructions, this still benefits from
            // instruction-level parallelism, since unlike in decoding, all subtractions
            // within a chunk are truly independent of each other.
            let output = &mut result[start_len..byte_count + start_len];
            let diff_len = byte_count - samples;
            let remainder_len = diff_len % 16;
            // Process full 16-byte chunks in reverse.
            // We iterate from the end so that predecessor values (at lower indices)
            // are still in their original state when we read them.
            let mut chunk_end = byte_count;
            while chunk_end >= samples + remainder_len + 16 {
                let chunk_start = chunk_end - 16;
                let prev_start = chunk_start - samples;

                // Read all values in the chunk and their predecessors first.
                // This makes all subtractions below independent, enabling vectorization.
                let curr: [u8; 16] = output[chunk_start..chunk_end].try_into().unwrap();
                let prev: [u8; 16] = output[prev_start..prev_start + 16].try_into().unwrap();

                let chunk = &mut output[chunk_start..chunk_end];
                chunk[0] = curr[0].wrapping_sub(prev[0]);
                chunk[1] = curr[1].wrapping_sub(prev[1]);
                chunk[2] = curr[2].wrapping_sub(prev[2]);
                chunk[3] = curr[3].wrapping_sub(prev[3]);
                chunk[4] = curr[4].wrapping_sub(prev[4]);
                chunk[5] = curr[5].wrapping_sub(prev[5]);
                chunk[6] = curr[6].wrapping_sub(prev[6]);
                chunk[7] = curr[7].wrapping_sub(prev[7]);
                chunk[8] = curr[8].wrapping_sub(prev[8]);
                chunk[9] = curr[9].wrapping_sub(prev[9]);
                chunk[10] = curr[10].wrapping_sub(prev[10]);
                chunk[11] = curr[11].wrapping_sub(prev[11]);
                chunk[12] = curr[12].wrapping_sub(prev[12]);
                chunk[13] = curr[13].wrapping_sub(prev[13]);
                chunk[14] = curr[14].wrapping_sub(prev[14]);
                chunk[15] = curr[15].wrapping_sub(prev[15]);

                chunk_end = chunk_start;
            }
            // Handle the leading remainder that doesn't fill a complete chunk of 16.
            // This must be done last (after the reverse chunk loop) because these are
            // the lowest indices, and the chunks above may read predecessor values
            // from this range.
            for i in (samples..samples + remainder_len).rev() {
                output[i] = output[i].wrapping_sub(output[i - samples]);
            }
        }
    };
}

fp_predict!(fp_predict_f32, f32);
fp_predict!(fp_predict_f64, f64);

macro_rules! integer_horizontal_predict {
    () => {
        fn horizontal_predict(row: &[Self::Inner], result: &mut Vec<Self::Inner>) {
            let sample_size = Self::SAMPLE_FORMAT.len();

            if row.len() < sample_size {
                debug_assert!(false);
                return;
            }

            let (start, rest) = row.split_at(sample_size);

            result.extend_from_slice(start);
            if result.capacity() - result.len() < rest.len() {
                return;
            }

            result.extend(
                row.into_iter()
                    .zip(rest)
                    .map(|(prev, current)| current.wrapping_sub(*prev)),
            );
        }

        fn floating_point_predict(_: &[Self::Inner], _: &mut Vec<u8>) {
            unreachable!("floating-point predictor is only valid for floating-point sample types")
        }
    };
}

/// Trait for different colortypes that can be encoded.
pub trait ColorType {
    /// The type of each sample of this colortype
    type Inner: super::TiffValue;
    /// The value of the tiff tag `PhotometricInterpretation`
    const TIFF_VALUE: PhotometricInterpretation;
    /// The value of the tiff tag `BitsPerSample`
    const BITS_PER_SAMPLE: &'static [u16];
    /// The value of the tiff tag `SampleFormat`
    const SAMPLE_FORMAT: &'static [SampleFormat];

    fn horizontal_predict(row: &[Self::Inner], result: &mut Vec<Self::Inner>);

    /// Apply floating-point predictor encoding to a row of samples.
    /// This is only implemented for floating-point types; integer types will panic.
    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>);
}

pub struct Gray8;
impl ColorType for Gray8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[8];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint];

    integer_horizontal_predict!();
}

pub struct GrayI8;
impl ColorType for GrayI8 {
    type Inner = i8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[8];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Int];

    integer_horizontal_predict!();
}

pub struct Gray16;
impl ColorType for Gray16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[16];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint];

    integer_horizontal_predict!();
}

pub struct GrayI16;
impl ColorType for GrayI16 {
    type Inner = i16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[16];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Int];

    integer_horizontal_predict!();
}

pub struct Gray32;
impl ColorType for Gray32 {
    type Inner = u32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint];

    integer_horizontal_predict!();
}

pub struct GrayI32;
impl ColorType for GrayI32 {
    type Inner = i32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Int];

    integer_horizontal_predict!();
}

pub struct Gray32Float;
impl ColorType for Gray32Float {
    type Inner = f32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f32(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct Gray64;
impl ColorType for Gray64 {
    type Inner = u64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint];

    integer_horizontal_predict!();
}

pub struct GrayI64;
impl ColorType for GrayI64 {
    type Inner = i64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Int];

    integer_horizontal_predict!();
}

pub struct Gray64Float;
impl ColorType for Gray64Float {
    type Inner = f64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f64(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct RGB8;
impl ColorType for RGB8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 3];

    integer_horizontal_predict!();
}

pub struct RGB16;
impl ColorType for RGB16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[16, 16, 16];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 3];

    integer_horizontal_predict!();
}

pub struct RGB32;
impl ColorType for RGB32 {
    type Inner = u32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 3];

    integer_horizontal_predict!();
}

pub struct RGB32Float;
impl ColorType for RGB32Float {
    type Inner = f32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP; 3];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f32(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct RGB64;
impl ColorType for RGB64 {
    type Inner = u64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 3];

    integer_horizontal_predict!();
}

pub struct RGB64Float;
impl ColorType for RGB64Float {
    type Inner = f64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP; 3];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f64(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct RGBA8;
impl ColorType for RGBA8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8, 8];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct RGBA16;
impl ColorType for RGBA16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[16, 16, 16, 16];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct RGBA32;
impl ColorType for RGBA32 {
    type Inner = u32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32, 32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct RGBA32Float;
impl ColorType for RGBA32Float {
    type Inner = f32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32, 32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP; 4];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f32(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct RGBA64;
impl ColorType for RGBA64 {
    type Inner = u64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64, 64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct RGBA64Float;
impl ColorType for RGBA64Float {
    type Inner = f64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::RGB;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64, 64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP; 4];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f64(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct CMYK8;
impl ColorType for CMYK8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8, 8];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct CMYK16;
impl ColorType for CMYK16 {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[16, 16, 16, 16];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct CMYK32;
impl ColorType for CMYK32 {
    type Inner = u32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32, 32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct CMYK32Float;
impl ColorType for CMYK32Float {
    type Inner = f32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[32, 32, 32, 32];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP; 4];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f32(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct CMYK64;
impl ColorType for CMYK64 {
    type Inner = u64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64, 64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 4];

    integer_horizontal_predict!();
}

pub struct CMYK64Float;
impl ColorType for CMYK64Float {
    type Inner = f64;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[64, 64, 64, 64];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP; 4];

    fn horizontal_predict(_: &[Self::Inner], _: &mut Vec<Self::Inner>) {
        unreachable!("horizontal predictor is not valid for floating-point sample types")
    }

    fn floating_point_predict(row: &[Self::Inner], result: &mut Vec<u8>) {
        fp_predict_f64(row, Self::SAMPLE_FORMAT.len(), result)
    }
}

pub struct YCbCr8;
impl ColorType for YCbCr8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::YCbCr;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 3];

    integer_horizontal_predict!();
}

pub struct CMYKA8;
impl ColorType for CMYKA8 {
    type Inner = u8;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::CMYK;
    const BITS_PER_SAMPLE: &'static [u16] = &[8, 8, 8, 8, 8];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; 5];

    integer_horizontal_predict!();
}
