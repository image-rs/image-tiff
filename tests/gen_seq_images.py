#!/usr/bin/env python3
"""Generate sequential-value TIFF test images for bit-error detection.

Uses manual TIFF structure writing to produce packed-sample images at
non-standard bit depths. Each image contains pixel values cycling 0..max,
so any bit-level decode error changes the checksum.

Images are 16x16 (256 pixels per channel).

Output: tests/images/seq-*.tiff + Rust test functions printed to stdout.
"""

import numpy as np
import struct
import os
from pathlib import Path

OUTDIR = Path(__file__).parent / "images"
OUTDIR.mkdir(exist_ok=True)

W, H = 16, 16
NPIX = W * H  # 256 pixels per channel


def sequential_values(max_val, count):
    """Generate cycling sequential values 0..max_val."""
    return [i % (max_val + 1) for i in range(count)]


def compute_sum(values):
    """Compute checksum as sum of all sample values."""
    return sum(values)


def pack_bits(values, bits, width):
    """Pack values into MSB-first bit stream, row by row.

    Each row is packed independently and padded to a byte boundary,
    matching TIFF strip/row semantics.
    """
    height = len(values) // width
    all_bytes = bytearray()
    for y in range(height):
        row_vals = values[y * width:(y + 1) * width]
        bit_buf = 0
        bit_count = 0
        for v in row_vals:
            bit_buf = (bit_buf << bits) | v
            bit_count += bits
            while bit_count >= 8:
                bit_count -= 8
                all_bytes.append((bit_buf >> bit_count) & 0xFF)
        if bit_count > 0:
            all_bytes.append((bit_buf << (8 - bit_count)) & 0xFF)
    return bytes(all_bytes)


def pack_bits_multichannel_contig(values, bits, width, channels):
    """Pack interleaved multichannel data: RGBRGBRGB...

    values is a flat list of length width*height*channels, already interleaved.
    Rows are packed independently.
    """
    height = len(values) // (width * channels)
    all_bytes = bytearray()
    for y in range(height):
        start = y * width * channels
        row_vals = values[start:start + width * channels]
        bit_buf = 0
        bit_count = 0
        for v in row_vals:
            bit_buf = (bit_buf << bits) | v
            bit_count += bits
            while bit_count >= 8:
                bit_count -= 8
                all_bytes.append((bit_buf >> bit_count) & 0xFF)
        if bit_count > 0:
            all_bytes.append((bit_buf << (8 - bit_count)) & 0xFF)
    return bytes(all_bytes)


def pack_bits_planar(values, bits, width, height, channels):
    """Pack planar data: all R samples, then all G, then all B.

    values is a flat list of length width*height*channels.
    First width*height are channel 0, next are channel 1, etc.
    Each plane's rows are packed independently.

    Returns (bytes, plane_sizes) where plane_sizes is a list of byte counts per plane.
    """
    plane_size = width * height
    all_bytes = bytearray()
    plane_byte_sizes = []
    for c in range(channels):
        plane = values[c * plane_size:(c + 1) * plane_size]
        plane_bytes = pack_bits(plane, bits, width)
        plane_byte_sizes.append(len(plane_bytes))
        all_bytes.extend(plane_bytes)
    return bytes(all_bytes), plane_byte_sizes


def apply_horizontal_predictor(values, bits, width):
    """Apply TIFF horizontal differencing predictor.

    For each row, first sample is unchanged, subsequent samples become
    (current - previous) mod 2^bits.
    """
    height = len(values) // width
    mod = 1 << bits
    result = list(values)
    for y in range(height):
        for x in range(width - 1, 0, -1):
            idx = y * width + x
            result[idx] = (result[idx] - result[idx - 1]) % mod
    return result


def apply_horizontal_predictor_rgb(values, bits, width, channels):
    """Apply horizontal predictor for interleaved RGB data."""
    height = len(values) // (width * channels)
    mod = 1 << bits
    result = list(values)
    stride = width * channels
    for y in range(height):
        row_start = y * stride
        # Work backwards to avoid overwriting values we still need
        for x in range(width - 1, 0, -1):
            for c in range(channels):
                idx = row_start + x * channels + c
                prev = row_start + (x - 1) * channels + c
                result[idx] = (result[idx] - result[prev]) % mod
    return result


def write_tiff(path, raw_data, width, height, bits, photometric=1,
               samples_per_pixel=1, planar=False, predictor=1,
               plane_sizes=None):
    """Write a valid TIFF file with precise IFD structure.

    photometric: 0=WhiteIsZero, 1=BlackIsZero, 2=RGB
    plane_sizes: list of byte sizes per plane (for planar images)
    """
    # For planar images, we need separate strips per plane
    if planar and samples_per_pixel > 1:
        num_strips = samples_per_pixel
        if plane_sizes is None:
            plane_size = len(raw_data) // samples_per_pixel
            plane_sizes = [plane_size] * samples_per_pixel
    else:
        num_strips = 1
        plane_sizes = [len(raw_data)]

    # Collect IFD entries as (tag, type_id, count, inline_or_None, offset_data_or_None)
    # We'll resolve offsets in a second pass
    entries = []

    def add_short(tag, value):
        entries.append((tag, 3, 1, value, None))

    add_short(256, width)                  # ImageWidth
    add_short(257, height)                 # ImageLength
    add_short(259, 1)                      # Compression = None
    add_short(262, photometric)            # PhotometricInterpretation
    add_short(278, height)                 # RowsPerStrip = all rows
    add_short(296, 1)                      # ResolutionUnit = None

    if samples_per_pixel > 1:
        add_short(277, samples_per_pixel)  # SamplesPerPixel
        add_short(284, 2 if planar else 1) # PlanarConfiguration

    if predictor > 1:
        add_short(317, predictor)          # Predictor

    # These need offset data; will be filled in during layout
    # Use sentinel tag numbers, actual entries added below
    entries.sort(key=lambda e: e[0])

    # Now compute layout
    # We need to know all entries to compute IFD size
    # Extra entries: BitsPerSample(258), StripOffsets(273), StripByteCounts(279),
    #                XResolution(282), YResolution(283)
    total_entries = len(entries) + 5

    ifd_offset = 8
    ifd_size = 2 + total_entries * 12 + 4

    extra_start = ifd_offset + ifd_size
    extra = bytearray()

    # BitsPerSample
    if samples_per_pixel > 1:
        bps_offset = extra_start + len(extra)
        for _ in range(samples_per_pixel):
            extra.extend(struct.pack('<H', bits))
        entries.append((258, 3, samples_per_pixel, None, bps_offset))
    else:
        entries.append((258, 3, 1, bits, None))

    # StripOffsets - will point to strip data
    # For multi-strip, we need an array of LONGs in extra data
    strip_data_start = None  # resolved after extra is complete

    if num_strips == 1:
        # Single strip: offset stored inline (resolved later)
        entries.append((273, 4, 1, 'STRIP_OFFSET_PLACEHOLDER', None))
    else:
        # Multiple strips: offset array in extra data
        strip_offsets_offset = extra_start + len(extra)
        # Reserve space for strip offset LONGs (filled after we know strip_data_start)
        strip_offsets_pos_in_extra = len(extra)
        for _ in range(num_strips):
            extra.extend(struct.pack('<I', 0))  # placeholder
        entries.append((273, 4, num_strips, None, strip_offsets_offset))

    # StripByteCounts
    if num_strips == 1:
        entries.append((279, 4, 1, plane_sizes[0], None))
    else:
        sbc_offset = extra_start + len(extra)
        for ps in plane_sizes:
            extra.extend(struct.pack('<I', ps))
        entries.append((279, 4, num_strips, None, sbc_offset))

    # XResolution
    xres_offset = extra_start + len(extra)
    extra.extend(struct.pack('<II', 72, 1))
    entries.append((282, 5, 1, None, xres_offset))

    # YResolution
    yres_offset = extra_start + len(extra)
    extra.extend(struct.pack('<II', 72, 1))
    entries.append((283, 5, 1, None, yres_offset))

    entries.sort(key=lambda e: e[0])

    # Strip data starts after extra
    strip_data_start = extra_start + len(extra)

    # Fill in strip offset placeholders
    if num_strips == 1:
        # Update the inline value for tag 273
        for i, (tag, typ, cnt, val, off) in enumerate(entries):
            if tag == 273:
                entries[i] = (273, 4, 1, strip_data_start, None)
                break
    else:
        # Fill in the strip offset array in extra data
        offset = strip_data_start
        for s in range(num_strips):
            pos = strip_offsets_pos_in_extra + s * 4
            struct.pack_into('<I', extra, pos, offset)
            offset += plane_sizes[s]

    # Build the file
    f = bytearray()

    # Header
    f.extend(b'II')
    f.extend(struct.pack('<H', 42))
    f.extend(struct.pack('<I', ifd_offset))

    # IFD
    f.extend(struct.pack('<H', len(entries)))

    for tag, typ, count, inline_val, offset_val in entries:
        f.extend(struct.pack('<H', tag))
        f.extend(struct.pack('<H', typ))
        f.extend(struct.pack('<I', count))

        if inline_val is not None:
            # Value fits inline
            if typ == 3:  # SHORT
                f.extend(struct.pack('<H', inline_val))
                f.extend(b'\x00\x00')
            elif typ == 4:  # LONG
                f.extend(struct.pack('<I', inline_val))
        else:
            # Value is at offset
            f.extend(struct.pack('<I', offset_val))

    # Next IFD = 0
    f.extend(struct.pack('<I', 0))

    # Extra data
    f.extend(extra)

    # Strip data
    f.extend(raw_data)

    with open(path, 'wb') as fh:
        fh.write(f)


def packed_byte_sum(values, bits, width):
    """Compute the sum of packed bytes containing the given sample values.

    For sub-byte depths, the decoder returns packed bytes, not individual samples.
    """
    raw = pack_bits(values, bits, width)
    return sum(raw)


def gen_gray(bits, white_is_zero=False, h_predictor=False):
    """Generate a grayscale image at any bit depth."""
    max_val = (1 << bits) - 1
    vals = sequential_values(max_val, NPIX)

    # For WhiteIsZero, the stored pixel values are inverted
    # The decoder should invert them back, producing original sequential values
    if white_is_zero:
        stored = [max_val - v for v in vals]
    else:
        stored = list(vals)

    # Apply predictor before packing (predictor operates on sample values)
    if h_predictor:
        stored = apply_horizontal_predictor(stored, bits, W)

    raw = pack_bits(stored, bits, W)

    photometric = 0 if white_is_zero else 1
    predictor_tag = 2 if h_predictor else 1

    suffix = ""
    if white_is_zero:
        suffix = "-miniswhite"
    elif h_predictor:
        suffix = "-hpredict"

    name = f"seq-1c-{bits}b{suffix}.tiff"
    path = OUTDIR / name

    write_tiff(path, raw, W, H, bits, photometric=photometric,
               predictor=predictor_tag)

    # Compute expected checksum from the decoder's perspective.
    # For sub-byte depths (<=8): decoder returns packed bytes, so checksum
    # is the sum of the packed byte values.
    # For supra-byte depths: decoder unpacks to u16/u32 container, so
    # checksum is the sum of sample values.
    if bits <= 8:
        # After decoding: WhiteIsZero gets inverted back to original,
        # predictor gets undone. Decoded samples = vals (the sequential values).
        checksum = packed_byte_sum(vals, bits, W)
        rust_fn = "test_image_sum_u8"
    elif bits <= 16:
        checksum = compute_sum(vals)
        rust_fn = "test_image_sum_u16"
    else:
        checksum = compute_sum(vals)
        rust_fn = "test_image_sum_u32"

    color = f"Gray({bits})"
    return name, rust_fn, color, checksum


def gen_rgb_contig(bits):
    """Generate an RGB contiguous (chunky) image."""
    max_val = (1 << bits) - 1
    vals = sequential_values(max_val, NPIX * 3)
    checksum = compute_sum(vals)

    raw = pack_bits_multichannel_contig(vals, bits, W, 3)

    name = f"seq-3c-{bits}b-contig.tiff"
    path = OUTDIR / name

    write_tiff(path, raw, W, H, bits, photometric=2,
               samples_per_pixel=3)

    if bits <= 8:
        rust_fn = "test_image_sum_u8"
    elif bits <= 16:
        rust_fn = "test_image_sum_u16"
    else:
        rust_fn = "test_image_sum_u32"

    color = f"RGB({bits})"
    return name, rust_fn, color, checksum


def gen_rgb_planar(bits):
    """Generate an RGB planar (separate) image."""
    max_val = (1 << bits) - 1
    # For planar: values are R plane, then G plane, then B plane
    # Each plane has NPIX values
    vals_r = sequential_values(max_val, NPIX)
    vals_g = sequential_values(max_val, NPIX)
    vals_b = sequential_values(max_val, NPIX)
    # Use different starting points for each plane to differentiate them
    vals_g = [(v + 100) % (max_val + 1) for v in vals_g]
    vals_b = [(v + 200) % (max_val + 1) for v in vals_b]

    all_vals = vals_r + vals_g + vals_b

    raw, plane_sizes = pack_bits_planar(all_vals, bits, W, H, 3)

    name = f"seq-3c-{bits}b-planar.tiff"
    path = OUTDIR / name

    write_tiff(path, raw, W, H, bits, photometric=2,
               samples_per_pixel=3, planar=True, plane_sizes=plane_sizes)

    # read_image() only returns the first plane for planar images.
    # Checksum is just the R plane's sum.
    # colortype() still reports RGB(N) (tested by existing planar tests).
    if bits <= 16:
        checksum = compute_sum(vals_r)
        rust_fn = "test_image_sum_u16"
    else:
        checksum = compute_sum(vals_r)
        rust_fn = "test_image_sum_u32"

    color = f"RGB({bits})"
    return name, rust_fn, color, checksum


def verify_with_tifffile(path, expected_vals, bits, description):
    """Verify the generated TIFF reads back correctly with tifffile."""
    try:
        import tifffile
        with tifffile.TiffFile(str(path)) as t:
            page = t.pages[0]
            assert page.bitspersample == bits, \
                f"{description}: BitsPerSample={page.bitspersample}, expected {bits}"
            # Can't always read back sub-byte packed data with tifffile,
            # but we can verify the structure
            return True
    except Exception as e:
        print(f"  WARNING: tifffile verification of {description} failed: {e}")
        return False


def main():
    results = []

    print("Generating test images...\n")

    # Sub-byte grayscale (BlackIsZero)
    for bits in [3, 5, 7]:
        r = gen_gray(bits)
        print(f"  {r[0]}: sum={r[3]}")
        results.append(r)

    # Sub-byte grayscale (WhiteIsZero)
    for bits in [3, 5, 6, 7]:
        r = gen_gray(bits, white_is_zero=True)
        print(f"  {r[0]}: sum={r[3]}")
        results.append(r)

    # Extended grayscale (BlackIsZero)
    for bits in [10, 12, 14, 24]:
        r = gen_gray(bits)
        print(f"  {r[0]}: sum={r[3]}")
        results.append(r)

    # Extended grayscale (WhiteIsZero)
    for bits in [10, 12, 14, 24]:
        r = gen_gray(bits, white_is_zero=True)
        print(f"  {r[0]}: sum={r[3]}")
        results.append(r)

    # Extended grayscale with horizontal predictor
    for bits in [10, 12, 14, 24]:
        r = gen_gray(bits, h_predictor=True)
        print(f"  {r[0]}: sum={r[3]}")
        results.append(r)

    # RGB contiguous
    for bits in [10, 12, 14, 24]:
        r = gen_rgb_contig(bits)
        print(f"  {r[0]}: sum={r[3]}")
        results.append(r)

    # RGB planar
    for bits in [10, 12, 14, 24]:
        r = gen_rgb_planar(bits)
        print(f"  {r[0]}: sum={r[3]}")
        results.append(r)

    # Verify structure with tifffile
    print("\nVerifying TIFF structure...")
    for name, _, _, _ in results:
        path = OUTDIR / name
        bits = int(name.split('-')[2].replace('b', '').split('.')[0])
        verify_with_tifffile(path, None, bits, name)

    # Print Rust test functions
    print("\n// ===== Sequential-value test functions =====")
    print("// Generated by gen_seq_images.py")
    print("// Each image has cycling 0..max values; any bit error changes the sum.")
    print()

    categories = [
        ("Sub-byte BlackIsZero", []),
        ("Sub-byte WhiteIsZero", []),
        ("Extended BlackIsZero", []),
        ("Extended WhiteIsZero", []),
        ("Extended HPredict", []),
        ("RGB Contiguous", []),
        ("RGB Planar", []),
    ]

    for name, rust_fn, color, checksum in results:
        bits_str = name.split('-')[2] if '3c' not in name else name.split('-')[2]
        bits_val = int(bits_str.replace('b', '').split('.')[0])

        if "3c" in name and "contig" in name:
            categories[5][1].append((name, rust_fn, color, checksum))
        elif "3c" in name and "planar" in name:
            categories[6][1].append((name, rust_fn, color, checksum))
        elif "miniswhite" in name and bits_val < 8:
            categories[1][1].append((name, rust_fn, color, checksum))
        elif bits_val < 8:
            categories[0][1].append((name, rust_fn, color, checksum))
        elif "miniswhite" in name:
            categories[3][1].append((name, rust_fn, color, checksum))
        elif "hpredict" in name:
            categories[4][1].append((name, rust_fn, color, checksum))
        else:
            categories[2][1].append((name, rust_fn, color, checksum))

    for cat_name, items in categories:
        if not items:
            continue
        print(f"// --- {cat_name} ---")
        for name, rust_fn, color, checksum in items:
            test_name = name.replace(".tiff", "").replace("-", "_")
            print(f'#[test]')
            print(f'fn test_{test_name}() {{')
            print(f'    {rust_fn}("{name}", ColorType::{color}, {checksum});')
            print(f'}}')
            print()
        print()

    print(f"// Total: {len(results)} test images generated")


if __name__ == '__main__':
    main()
