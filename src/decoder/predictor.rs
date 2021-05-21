use crate::bytecast;

pub trait Wrapping {
    fn wrapping_add(&self, other: Self) -> Self;
}

impl Wrapping for u8 {
    fn wrapping_add(&self, other: Self) -> Self {
        u8::wrapping_add(*self, other)
    }
}

impl Wrapping for u16 {
    fn wrapping_add(&self, other: Self) -> Self {
        u16::wrapping_add(*self, other)
    }
}

impl Wrapping for u32 {
    fn wrapping_add(&self, other: Self) -> Self {
        u32::wrapping_add(*self, other)
    }
}

impl Wrapping for u64 {
    fn wrapping_add(&self, other: Self) -> Self {
        u64::wrapping_add(*self, other)
    }
}

impl Wrapping for i8 {
    fn wrapping_add(&self, other: Self) -> Self {
        i8::wrapping_add(*self, other)
    }
}

impl Wrapping for i16 {
    fn wrapping_add(&self, other: Self) -> Self {
        i16::wrapping_add(*self, other)
    }
}

impl Wrapping for i32 {
    fn wrapping_add(&self, other: Self) -> Self {
        i32::wrapping_add(*self, other)
    }
}

impl Wrapping for i64 {
    fn wrapping_add(&self, other: Self) -> Self {
        i64::wrapping_add(*self, other)
    }
}

pub fn rev_hpredict_nsamp<T>(
    image: &mut [T],
    (width, height): (usize, usize), // Size of the block
    img_width: usize, // Width of the decoding result (this distinction is needed for tiles)
    samples: usize,
) where
    T: Copy + Wrapping,
{
    for row in 0..height {
        for col in samples..width * samples {
            let prev_pixel = image[(row * img_width * samples + col - samples)];
            let pixel = &mut image[(row * img_width * samples + col)];
            *pixel = pixel.wrapping_add(prev_pixel);
        }
    }
}

pub fn fp_predict<T>(
    buf: &mut [T],
    copy_buf: &mut [u8],
    (width, height): (usize, usize),
    res_width: usize,
    samples_per_pixel: usize,
    byte_len: usize,
) where
    [T]: AsNeMutBytes,
    [T]: Reorder,
{
    for row in 0..height {
        // Calculate offset into the result buffer
        let scanline_start = row * res_width * samples_per_pixel;
        let row_buf = &mut buf[scanline_start..(scanline_start + width * samples_per_pixel)];
        let row_buf = AsNeMutBytes::as_ne_mut_bytes(row_buf);

        copy_buf.clone_from_slice(row_buf);

        // Horizontal differencing
        for pixel in 1..(width * byte_len) {
            for sample in 0..samples_per_pixel {
                let prev_pixel = copy_buf[(pixel - 1) * samples_per_pixel + sample];
                let curr_pixel = &mut copy_buf[pixel * samples_per_pixel + sample];

                *curr_pixel = curr_pixel.wrapping_add(prev_pixel);
            }
        }

        let row_increment = width * samples_per_pixel;
        Reorder::reorder(buf, &copy_buf, row_increment, scanline_start);
    }
}

pub trait AsNeMutBytes {
    fn as_ne_mut_bytes(&mut self) -> &mut [u8];
}

impl AsNeMutBytes for [f32] {
    fn as_ne_mut_bytes(&mut self) -> &mut [u8] {
        bytecast::f32_as_ne_mut_bytes(self)
    }
}

impl AsNeMutBytes for [f64] {
    fn as_ne_mut_bytes(&mut self) -> &mut [u8] {
        bytecast::f64_as_ne_mut_bytes(self)
    }
}

pub trait Reorder {
    fn reorder(buf: &mut Self, copy: &[u8], row_increment: usize, scanline_start: usize);
}

impl Reorder for [f32] {
    fn reorder(buf: &mut Self, copy: &[u8], row_increment: usize, scanline_start: usize) {
        for sample in 0..row_increment {
            // TODO: use f32::from_be_bytes() when we can (version 1.40)
            buf[scanline_start + sample] = f32::from_bits(u32::from_be_bytes([
                copy[row_increment * 0 + sample],
                copy[row_increment * 1 + sample],
                copy[row_increment * 2 + sample],
                copy[row_increment * 3 + sample],
            ]));
        }
    }
}

impl Reorder for [f64] {
    fn reorder(buf: &mut Self, copy: &[u8], row_increment: usize, scanline_start: usize) {
        for sample in 0..row_increment {
            // TODO: use f64::from_be_bytes() when we can (version 1.40)
            buf[scanline_start + sample] = f64::from_bits(u64::from_be_bytes([
                copy[row_increment * 0 + sample],
                copy[row_increment * 1 + sample],
                copy[row_increment * 2 + sample],
                copy[row_increment * 3 + sample],
                copy[row_increment * 4 + sample],
                copy[row_increment * 5 + sample],
                copy[row_increment * 6 + sample],
                copy[row_increment * 7 + sample],
            ]));
        }
    }
}
