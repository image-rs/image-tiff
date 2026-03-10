/*
 * gen_seq_tiff.c — Generate sequential-value TIFF test images using libtiff.
 *
 * Each image is 16x16 with sample values cycling 0..max.
 * Any single-bit decode error changes the pixel-value checksum.
 *
 * Usage: ./gen_seq_tiff <output_dir>
 *
 * Compile: cc -O2 -o gen_seq_tiff gen_seq_tiff.c -ltiff -lm
 */

#include <tiffio.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <math.h>

#define W 16
#define H 16
#define NPIX (W * H)

/* Pack a single sample value into an MSB-first bit stream at the given bit offset. */
static void pack_value(uint8_t *buf, int *bit_offset, uint32_t val, int bits)
{
    /* Write bits MSB-first */
    for (int i = bits - 1; i >= 0; i--) {
        int byte_idx = *bit_offset / 8;
        int bit_in_byte = *bit_offset % 8;
        if ((val >> i) & 1)
            buf[byte_idx] |= (uint8_t)(0x80 >> bit_in_byte);
        (*bit_offset)++;
    }
}

/* Write a single-channel grayscale TIFF with N-bit packed samples.
 * libtiff expects packed MSB-first data for all non-standard bit depths. */
static int write_gray(const char *path, int bits,
                      uint16_t photometric, uint16_t predictor)
{
    TIFF *tif = TIFFOpen(path, "w");
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, photometric);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);
    if (predictor != PREDICTOR_NONE)
        TIFFSetField(tif, TIFFTAG_PREDICTOR, predictor);

    uint32_t max_val = (1u << bits) - 1;
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        int bit_offset = 0;

        for (int x = 0; x < W; x++) {
            uint32_t val = (y * W + x) % (max_val + 1);
            /* For WhiteIsZero, store inverted values (decoder will invert back) */
            if (photometric == PHOTOMETRIC_MINISWHITE)
                val = max_val - val;
            pack_value(buf, &bit_offset, val, bits);
        }

        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf);
            TIFFClose(tif);
            return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a 3-channel RGB TIFF with N-bit packed samples, contiguous layout. */
static int write_rgb_contig(const char *path, int bits)
{
    TIFF *tif = TIFFOpen(path, "w");
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 3);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_RGB);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    uint32_t max_val = (1u << bits) - 1;
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        int bit_offset = 0;

        for (int x = 0; x < W; x++) {
            for (int c = 0; c < 3; c++) {
                uint32_t idx = (y * W + x) * 3 + c;
                uint32_t val = idx % (max_val + 1);
                pack_value(buf, &bit_offset, val, bits);
            }
        }

        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf);
            TIFFClose(tif);
            return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a 3-channel RGB TIFF with N-bit packed samples, planar (separate) layout. */
static int write_rgb_planar(const char *path, int bits)
{
    TIFF *tif = TIFFOpen(path, "w");
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 3);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_RGB);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_SEPARATE);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    uint32_t max_val = (1u << bits) - 1;
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int s = 0; s < 3; s++) {
        for (int y = 0; y < H; y++) {
            memset(buf, 0, scanline_size);
            int bit_offset = 0;

            for (int x = 0; x < W; x++) {
                /* Plane 0: cycling 0..max, plane 1: +100, plane 2: +200 */
                uint32_t val = ((y * W + x) + s * 100) % (max_val + 1);
                pack_value(buf, &bit_offset, val, bits);
            }

            if (TIFFWriteScanline(tif, buf, y, (uint16_t)s) < 0) {
                fprintf(stderr, "Error writing scanline %d plane %d to %s\n",
                        y, s, path);
                _TIFFfree(buf);
                TIFFClose(tif);
                return -1;
            }
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a standard-depth grayscale TIFF with specified compression.
 * Works for 8-bit and 16-bit depths using native sample storage. */
static int write_gray_compressed(const char *path, int bits,
                                 uint16_t compression, uint16_t predictor,
                                 const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, compression);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);
    if (predictor != PREDICTOR_NONE)
        TIFFSetField(tif, TIFFTAG_PREDICTOR, predictor);

    uint32_t max_val = (bits < 32) ? ((1u << bits) - 1) : 0xFFFFFFFFu;
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        for (int x = 0; x < W; x++) {
            uint32_t val = (y * W + x) % (max_val + 1);
            if (bits == 8) {
                buf[x] = (uint8_t)val;
            } else if (bits == 16) {
                ((uint16_t *)buf)[x] = (uint16_t)val;
            } else if (bits == 32) {
                ((uint32_t *)buf)[x] = val;
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a standard-depth RGB TIFF with specified compression. */
static int write_rgb_compressed(const char *path, int bits,
                                uint16_t compression, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 3);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_RGB);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, compression);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    uint32_t max_val = (bits < 32) ? ((1u << bits) - 1) : 0xFFFFFFFFu;
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        for (int x = 0; x < W; x++) {
            for (int c = 0; c < 3; c++) {
                uint32_t idx = (y * W + x) * 3 + c;
                uint32_t val = idx % (max_val + 1);
                if (bits == 8) {
                    buf[x * 3 + c] = (uint8_t)val;
                } else if (bits == 16) {
                    ((uint16_t *)buf)[x * 3 + c] = (uint16_t)val;
                }
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a tiled grayscale TIFF. Uses 8x8 tiles. */
static int write_gray_tiled(const char *path, int bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    int tw = 16, th = 16;
    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_TILEWIDTH, tw);
    TIFFSetField(tif, TIFFTAG_TILELENGTH, th);

    uint32_t max_val = (bits < 32) ? ((1u << bits) - 1) : 0xFFFFFFFFu;
    tsize_t tile_size = TIFFTileSize(tif);
    uint8_t *tile_buf = (uint8_t *)_TIFFmalloc(tile_size);
    if (!tile_buf) { TIFFClose(tif); return -1; }

    for (int ty = 0; ty < H; ty += th) {
        for (int tx = 0; tx < W; tx += tw) {
            memset(tile_buf, 0, tile_size);
            for (int row = 0; row < th; row++) {
                for (int col = 0; col < tw; col++) {
                    int y = ty + row, x = tx + col;
                    uint32_t val = (y * W + x) % (max_val + 1);
                    if (bits == 8) {
                        tile_buf[row * tw + col] = (uint8_t)val;
                    } else if (bits == 16) {
                        ((uint16_t *)tile_buf)[row * tw + col] = (uint16_t)val;
                    }
                }
            }
            if (TIFFWriteTile(tif, tile_buf, tx, ty, 0, 0) < 0) {
                fprintf(stderr, "Error writing tile at (%d,%d) to %s\n", tx, ty, path);
                _TIFFfree(tile_buf); TIFFClose(tif); return -1;
            }
        }
    }

    _TIFFfree(tile_buf);
    TIFFClose(tif);
    return 0;
}

/* Write a tiled RGB TIFF. Uses 8x8 tiles. */
static int write_rgb_tiled(const char *path, int bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    int tw = 16, th = 16;
    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 3);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_RGB);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_TILEWIDTH, tw);
    TIFFSetField(tif, TIFFTAG_TILELENGTH, th);

    uint32_t max_val = (bits < 32) ? ((1u << bits) - 1) : 0xFFFFFFFFu;
    tsize_t tile_size = TIFFTileSize(tif);
    uint8_t *tile_buf = (uint8_t *)_TIFFmalloc(tile_size);
    if (!tile_buf) { TIFFClose(tif); return -1; }

    for (int ty = 0; ty < H; ty += th) {
        for (int tx = 0; tx < W; tx += tw) {
            memset(tile_buf, 0, tile_size);
            for (int row = 0; row < th; row++) {
                for (int col = 0; col < tw; col++) {
                    int y = ty + row, x = tx + col;
                    for (int c = 0; c < 3; c++) {
                        uint32_t idx = (y * W + x) * 3 + c;
                        uint32_t val = idx % (max_val + 1);
                        if (bits == 8) {
                            tile_buf[(row * tw + col) * 3 + c] = (uint8_t)val;
                        } else if (bits == 16) {
                            ((uint16_t *)tile_buf)[(row * tw + col) * 3 + c] = (uint16_t)val;
                        }
                    }
                }
            }
            if (TIFFWriteTile(tif, tile_buf, tx, ty, 0, 0) < 0) {
                fprintf(stderr, "Error writing tile at (%d,%d) to %s\n", tx, ty, path);
                _TIFFfree(tile_buf); TIFFClose(tif); return -1;
            }
        }
    }

    _TIFFfree(tile_buf);
    TIFFClose(tif);
    return 0;
}

/* Write a grayscale float TIFF (32-bit or 64-bit). Values are (y*W+x)/255.0 */
static int write_gray_float(const char *path, int float_bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, float_bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_IEEEFP);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        for (int x = 0; x < W; x++) {
            double val = (double)(y * W + x) / 255.0;
            if (float_bits == 32) {
                ((float *)buf)[x] = (float)val;
            } else {
                ((double *)buf)[x] = val;
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write an RGB float TIFF (32-bit). Values are idx/767.0 where idx = (y*W+x)*3+c */
static int write_rgb_float(const char *path, int float_bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 3);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, float_bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_IEEEFP);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_RGB);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        for (int x = 0; x < W; x++) {
            for (int c = 0; c < 3; c++) {
                double val = (double)((y * W + x) * 3 + c) / 767.0;
                if (float_bits == 32) {
                    ((float *)buf)[x * 3 + c] = (float)val;
                } else {
                    ((double *)buf)[x * 3 + c] = val;
                }
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a signed integer grayscale TIFF. Values cycle -128..127 for 8-bit,
 * -32768..32767 for 16-bit. */
static int write_gray_signed(const char *path, int bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_INT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        for (int x = 0; x < W; x++) {
            int32_t pixel_idx = y * W + x;
            if (bits == 8) {
                /* Cycle -128..127: signed_val = (pixel_idx % 256) - 128 */
                int8_t val = (int8_t)((pixel_idx % 256) - 128);
                ((int8_t *)buf)[x] = val;
            } else if (bits == 16) {
                /* Same pattern but 16-bit signed: cycle through -128..127 for simplicity */
                int16_t val = (int16_t)((pixel_idx % 256) - 128);
                ((int16_t *)buf)[x] = val;
            } else if (bits == 32) {
                int32_t val = (int32_t)((pixel_idx % 256) - 128);
                ((int32_t *)buf)[x] = val;
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a 4-channel CMYK TIFF. Values cycle 0..max per channel. */
static int write_cmyk(const char *path, int bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 4);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_SEPARATED);
    TIFFSetField(tif, TIFFTAG_INKSET, INKSET_CMYK);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    uint32_t max_val = (bits < 32) ? ((1u << bits) - 1) : 0xFFFFFFFFu;
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        for (int x = 0; x < W; x++) {
            for (int c = 0; c < 4; c++) {
                uint32_t idx = (y * W + x) * 4 + c;
                uint32_t val = idx % (max_val + 1);
                if (bits == 8) {
                    buf[x * 4 + c] = (uint8_t)val;
                } else if (bits == 16) {
                    ((uint16_t *)buf)[x * 4 + c] = (uint16_t)val;
                }
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a palette (indexed color) TIFF with 8-bit indices and a known colormap.
 * Colormap: R[i] = i*257, G[i] = (255-i)*257, B[i] = (i*37 % 256)*257
 * (multiplied by 257 for 16-bit colormap entries per TIFF spec). */
static int write_palette(const char *path, int bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    int ncolors = 1 << bits;

    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_PALETTE);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    /* Build colormap — 3 arrays of ncolors uint16 entries */
    uint16_t *r = (uint16_t *)calloc(ncolors, sizeof(uint16_t));
    uint16_t *g = (uint16_t *)calloc(ncolors, sizeof(uint16_t));
    uint16_t *b = (uint16_t *)calloc(ncolors, sizeof(uint16_t));
    if (!r || !g || !b) {
        free(r); free(g); free(b); TIFFClose(tif); return -1;
    }
    for (int i = 0; i < ncolors; i++) {
        r[i] = (uint16_t)((i % 256) * 257);
        g[i] = (uint16_t)(((255 - (i % 256))) * 257);
        b[i] = (uint16_t)(((i * 37) % 256) * 257);
    }
    TIFFSetField(tif, TIFFTAG_COLORMAP, r, g, b);

    /* Indices cycle 0..ncolors-1 */
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { free(r); free(g); free(b); TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        if (bits == 8) {
            for (int x = 0; x < W; x++)
                buf[x] = (uint8_t)((y * W + x) % ncolors);
        } else if (bits == 4) {
            /* Pack two 4-bit indices per byte, MSB first */
            for (int x = 0; x < W; x++) {
                uint8_t idx = (uint8_t)((y * W + x) % ncolors);
                if (x % 2 == 0)
                    buf[x / 2] = (uint8_t)(idx << 4);
                else
                    buf[x / 2] |= idx;
            }
        } else if (bits == 1) {
            /* Pack 1-bit indices */
            int bit_offset = 0;
            for (int x = 0; x < W; x++) {
                uint32_t idx = (y * W + x) % ncolors;
                pack_value(buf, &bit_offset, idx, 1);
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); free(r); free(g); free(b); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    free(r); free(g); free(b);
    TIFFClose(tif);
    return 0;
}

/* Write an RGBA TIFF with associated alpha (pre-multiplied). */
static int write_rgba(const char *path, int bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    uint16_t extra_samples[] = { EXTRASAMPLE_ASSOCALPHA };
    TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
    TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
    TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 4);
    TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
    TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
    TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_RGB);
    TIFFSetField(tif, TIFFTAG_EXTRASAMPLES, 1, extra_samples);
    TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
    TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
    TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);

    uint32_t max_val = (bits < 32) ? ((1u << bits) - 1) : 0xFFFFFFFFu;
    tsize_t scanline_size = TIFFScanlineSize(tif);
    uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
    if (!buf) { TIFFClose(tif); return -1; }

    for (int y = 0; y < H; y++) {
        memset(buf, 0, scanline_size);
        for (int x = 0; x < W; x++) {
            for (int c = 0; c < 4; c++) {
                uint32_t idx = (y * W + x) * 4 + c;
                uint32_t val = idx % (max_val + 1);
                if (bits == 8) {
                    buf[x * 4 + c] = (uint8_t)val;
                } else if (bits == 16) {
                    ((uint16_t *)buf)[x * 4 + c] = (uint16_t)val;
                }
            }
        }
        if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
            fprintf(stderr, "Error writing scanline %d to %s\n", y, path);
            _TIFFfree(buf); TIFFClose(tif); return -1;
        }
    }

    _TIFFfree(buf);
    TIFFClose(tif);
    return 0;
}

/* Write a multi-page TIFF with 3 gray pages. Page n has values offset by n*50. */
static int write_multipage(const char *path, int bits, const char *mode)
{
    TIFF *tif = TIFFOpen(path, mode);
    if (!tif) { fprintf(stderr, "Cannot open %s\n", path); return -1; }

    uint32_t max_val = (bits < 32) ? ((1u << bits) - 1) : 0xFFFFFFFFu;
    int npages = 3;

    for (int page = 0; page < npages; page++) {
        TIFFSetField(tif, TIFFTAG_IMAGEWIDTH, W);
        TIFFSetField(tif, TIFFTAG_IMAGELENGTH, H);
        TIFFSetField(tif, TIFFTAG_SAMPLESPERPIXEL, 1);
        TIFFSetField(tif, TIFFTAG_BITSPERSAMPLE, bits);
        TIFFSetField(tif, TIFFTAG_SAMPLEFORMAT, SAMPLEFORMAT_UINT);
        TIFFSetField(tif, TIFFTAG_PHOTOMETRIC, PHOTOMETRIC_MINISBLACK);
        TIFFSetField(tif, TIFFTAG_COMPRESSION, COMPRESSION_NONE);
        TIFFSetField(tif, TIFFTAG_PLANARCONFIG, PLANARCONFIG_CONTIG);
        TIFFSetField(tif, TIFFTAG_ROWSPERSTRIP, H);
        TIFFSetField(tif, TIFFTAG_SUBFILETYPE, FILETYPE_PAGE);
        TIFFSetField(tif, TIFFTAG_PAGENUMBER, (uint16_t)page, (uint16_t)npages);

        tsize_t scanline_size = TIFFScanlineSize(tif);
        uint8_t *buf = (uint8_t *)_TIFFmalloc(scanline_size);
        if (!buf) { TIFFClose(tif); return -1; }

        for (int y = 0; y < H; y++) {
            memset(buf, 0, scanline_size);
            for (int x = 0; x < W; x++) {
                uint32_t val = ((y * W + x) + page * 50) % (max_val + 1);
                if (bits == 8) {
                    buf[x] = (uint8_t)val;
                } else if (bits == 16) {
                    ((uint16_t *)buf)[x] = (uint16_t)val;
                }
            }
            if (TIFFWriteScanline(tif, buf, y, 0) < 0) {
                fprintf(stderr, "Error writing scanline %d page %d to %s\n", y, page, path);
                _TIFFfree(buf); TIFFClose(tif); return -1;
            }
        }
        _TIFFfree(buf);

        if (!TIFFWriteDirectory(tif)) {
            fprintf(stderr, "Error writing directory page %d to %s\n", page, path);
            TIFFClose(tif); return -1;
        }
    }

    TIFFClose(tif);
    return 0;
}

int main(int argc, char **argv)
{
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <output_dir>\n", argv[0]);
        return 1;
    }

    const char *dir = argv[1];
    char path[512];
    int ok = 1;

    printf("Generating libtiff test images in %s/\n\n", dir);

    /* Sub-byte BlackIsZero */
    int subbyte_biz[] = {3, 5, 7};
    for (int i = 0; i < 3; i++) {
        int b = subbyte_biz[i];
        snprintf(path, sizeof(path), "%s/seq-1c-%db.tiff", dir, b);
        if (write_gray(path, b, PHOTOMETRIC_MINISBLACK, PREDICTOR_NONE) == 0)
            printf("  OK seq-1c-%db.tiff\n", b);
        else { printf("  FAIL seq-1c-%db.tiff\n", b); ok = 0; }
    }

    /* Sub-byte WhiteIsZero */
    int subbyte_wiz[] = {3, 5, 6, 7};
    for (int i = 0; i < 4; i++) {
        int b = subbyte_wiz[i];
        snprintf(path, sizeof(path), "%s/seq-1c-%db-miniswhite.tiff", dir, b);
        if (write_gray(path, b, PHOTOMETRIC_MINISWHITE, PREDICTOR_NONE) == 0)
            printf("  OK seq-1c-%db-miniswhite.tiff\n", b);
        else { printf("  FAIL seq-1c-%db-miniswhite.tiff\n", b); ok = 0; }
    }

    /* Extended BlackIsZero */
    int ext_bits[] = {10, 12, 14, 24};
    for (int i = 0; i < 4; i++) {
        int b = ext_bits[i];
        snprintf(path, sizeof(path), "%s/seq-1c-%db.tiff", dir, b);
        if (write_gray(path, b, PHOTOMETRIC_MINISBLACK, PREDICTOR_NONE) == 0)
            printf("  OK seq-1c-%db.tiff\n", b);
        else { printf("  FAIL seq-1c-%db.tiff\n", b); ok = 0; }
    }

    /* Extended WhiteIsZero */
    for (int i = 0; i < 4; i++) {
        int b = ext_bits[i];
        snprintf(path, sizeof(path), "%s/seq-1c-%db-miniswhite.tiff", dir, b);
        if (write_gray(path, b, PHOTOMETRIC_MINISWHITE, PREDICTOR_NONE) == 0)
            printf("  OK seq-1c-%db-miniswhite.tiff\n", b);
        else { printf("  FAIL seq-1c-%db-miniswhite.tiff\n", b); ok = 0; }
    }

    /* Extended HPredict — libtiff may not support predictor for packed non-standard depths.
     * Try it and report failure gracefully. */
    for (int i = 0; i < 4; i++) {
        int b = ext_bits[i];
        snprintf(path, sizeof(path), "%s/seq-1c-%db-hpredict.tiff", dir, b);
        if (write_gray(path, b, PHOTOMETRIC_MINISBLACK, PREDICTOR_HORIZONTAL) == 0)
            printf("  OK seq-1c-%db-hpredict.tiff\n", b);
        else { printf("  FAIL seq-1c-%db-hpredict.tiff (may not be supported)\n", b); }
    }

    /* RGB Contiguous */
    for (int i = 0; i < 4; i++) {
        int b = ext_bits[i];
        snprintf(path, sizeof(path), "%s/seq-3c-%db-contig.tiff", dir, b);
        if (write_rgb_contig(path, b) == 0)
            printf("  OK seq-3c-%db-contig.tiff\n", b);
        else { printf("  FAIL seq-3c-%db-contig.tiff\n", b); ok = 0; }
    }

    /* RGB Planar */
    for (int i = 0; i < 4; i++) {
        int b = ext_bits[i];
        snprintf(path, sizeof(path), "%s/seq-3c-%db-planar.tiff", dir, b);
        if (write_rgb_planar(path, b) == 0)
            printf("  OK seq-3c-%db-planar.tiff\n", b);
        else { printf("  FAIL seq-3c-%db-planar.tiff\n", b); ok = 0; }
    }

    printf("\n--- Standard depths with compression ---\n");

    /* Gray 8-bit: LZW, Deflate, PackBits */
    {
        struct { const char *suffix; uint16_t comp; } comps[] = {
            {"lzw",      COMPRESSION_LZW},
            {"deflate",  COMPRESSION_DEFLATE},
            {"packbits", COMPRESSION_PACKBITS},
        };
        for (int i = 0; i < 3; i++) {
            snprintf(path, sizeof(path), "%s/seq-1c-8b-%s.tiff", dir, comps[i].suffix);
            if (write_gray_compressed(path, 8, comps[i].comp, PREDICTOR_NONE, "w") == 0)
                printf("  OK seq-1c-8b-%s.tiff\n", comps[i].suffix);
            else { printf("  FAIL seq-1c-8b-%s.tiff\n", comps[i].suffix); ok = 0; }
        }
    }

    /* Gray 16-bit: LZW, Deflate */
    {
        struct { const char *suffix; uint16_t comp; } comps[] = {
            {"lzw",     COMPRESSION_LZW},
            {"deflate", COMPRESSION_DEFLATE},
        };
        for (int i = 0; i < 2; i++) {
            snprintf(path, sizeof(path), "%s/seq-1c-16b-%s.tiff", dir, comps[i].suffix);
            if (write_gray_compressed(path, 16, comps[i].comp, PREDICTOR_NONE, "w") == 0)
                printf("  OK seq-1c-16b-%s.tiff\n", comps[i].suffix);
            else { printf("  FAIL seq-1c-16b-%s.tiff\n", comps[i].suffix); ok = 0; }
        }
    }

    /* Gray 8-bit LZW with horizontal predictor */
    snprintf(path, sizeof(path), "%s/seq-1c-8b-lzw-hpredict.tiff", dir);
    if (write_gray_compressed(path, 8, COMPRESSION_LZW, PREDICTOR_HORIZONTAL, "w") == 0)
        printf("  OK seq-1c-8b-lzw-hpredict.tiff\n");
    else { printf("  FAIL seq-1c-8b-lzw-hpredict.tiff\n"); ok = 0; }

    /* RGB 8-bit: LZW */
    snprintf(path, sizeof(path), "%s/seq-3c-8b-lzw.tiff", dir);
    if (write_rgb_compressed(path, 8, COMPRESSION_LZW, "w") == 0)
        printf("  OK seq-3c-8b-lzw.tiff\n");
    else { printf("  FAIL seq-3c-8b-lzw.tiff\n"); ok = 0; }

    printf("\n--- Tiled images ---\n");

    /* Gray tiled: 8-bit and 16-bit */
    snprintf(path, sizeof(path), "%s/seq-1c-8b-tiled.tiff", dir);
    if (write_gray_tiled(path, 8, "w") == 0)
        printf("  OK seq-1c-8b-tiled.tiff\n");
    else { printf("  FAIL seq-1c-8b-tiled.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-1c-16b-tiled.tiff", dir);
    if (write_gray_tiled(path, 16, "w") == 0)
        printf("  OK seq-1c-16b-tiled.tiff\n");
    else { printf("  FAIL seq-1c-16b-tiled.tiff\n"); ok = 0; }

    /* RGB tiled: 8-bit */
    snprintf(path, sizeof(path), "%s/seq-3c-8b-tiled.tiff", dir);
    if (write_rgb_tiled(path, 8, "w") == 0)
        printf("  OK seq-3c-8b-tiled.tiff\n");
    else { printf("  FAIL seq-3c-8b-tiled.tiff\n"); ok = 0; }

    printf("\n--- BigTIFF ---\n");

    /* BigTIFF: gray 8-bit and RGB 16-bit */
    snprintf(path, sizeof(path), "%s/seq-1c-8b-bigtiff.tiff", dir);
    if (write_gray_compressed(path, 8, COMPRESSION_NONE, PREDICTOR_NONE, "w8") == 0)
        printf("  OK seq-1c-8b-bigtiff.tiff\n");
    else { printf("  FAIL seq-1c-8b-bigtiff.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-3c-16b-bigtiff.tiff", dir);
    if (write_rgb_compressed(path, 16, COMPRESSION_NONE, "w8") == 0)
        printf("  OK seq-3c-16b-bigtiff.tiff\n");
    else { printf("  FAIL seq-3c-16b-bigtiff.tiff\n"); ok = 0; }

    printf("\n--- Float samples ---\n");

    /* Gray float: 32-bit and 64-bit */
    snprintf(path, sizeof(path), "%s/seq-1c-32f.tiff", dir);
    if (write_gray_float(path, 32, "w") == 0)
        printf("  OK seq-1c-32f.tiff\n");
    else { printf("  FAIL seq-1c-32f.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-1c-64f.tiff", dir);
    if (write_gray_float(path, 64, "w") == 0)
        printf("  OK seq-1c-64f.tiff\n");
    else { printf("  FAIL seq-1c-64f.tiff\n"); ok = 0; }

    /* RGB float: 32-bit */
    snprintf(path, sizeof(path), "%s/seq-3c-32f.tiff", dir);
    if (write_rgb_float(path, 32, "w") == 0)
        printf("  OK seq-3c-32f.tiff\n");
    else { printf("  FAIL seq-3c-32f.tiff\n"); ok = 0; }

    printf("\n--- Signed integer ---\n");

    /* Signed gray: 8-bit, 16-bit, 32-bit */
    snprintf(path, sizeof(path), "%s/seq-1c-i8.tiff", dir);
    if (write_gray_signed(path, 8, "w") == 0)
        printf("  OK seq-1c-i8.tiff\n");
    else { printf("  FAIL seq-1c-i8.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-1c-i16.tiff", dir);
    if (write_gray_signed(path, 16, "w") == 0)
        printf("  OK seq-1c-i16.tiff\n");
    else { printf("  FAIL seq-1c-i16.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-1c-i32.tiff", dir);
    if (write_gray_signed(path, 32, "w") == 0)
        printf("  OK seq-1c-i32.tiff\n");
    else { printf("  FAIL seq-1c-i32.tiff\n"); ok = 0; }

    printf("\n--- CMYK ---\n");

    snprintf(path, sizeof(path), "%s/seq-4c-8b-cmyk.tiff", dir);
    if (write_cmyk(path, 8, "w") == 0)
        printf("  OK seq-4c-8b-cmyk.tiff\n");
    else { printf("  FAIL seq-4c-8b-cmyk.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-4c-16b-cmyk.tiff", dir);
    if (write_cmyk(path, 16, "w") == 0)
        printf("  OK seq-4c-16b-cmyk.tiff\n");
    else { printf("  FAIL seq-4c-16b-cmyk.tiff\n"); ok = 0; }

    printf("\n--- Palette (indexed) ---\n");

    snprintf(path, sizeof(path), "%s/seq-1c-8b-palette.tiff", dir);
    if (write_palette(path, 8, "w") == 0)
        printf("  OK seq-1c-8b-palette.tiff\n");
    else { printf("  FAIL seq-1c-8b-palette.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-1c-4b-palette.tiff", dir);
    if (write_palette(path, 4, "w") == 0)
        printf("  OK seq-1c-4b-palette.tiff\n");
    else { printf("  FAIL seq-1c-4b-palette.tiff\n"); ok = 0; }

    printf("\n--- RGBA (associated alpha) ---\n");

    snprintf(path, sizeof(path), "%s/seq-4c-8b-rgba.tiff", dir);
    if (write_rgba(path, 8, "w") == 0)
        printf("  OK seq-4c-8b-rgba.tiff\n");
    else { printf("  FAIL seq-4c-8b-rgba.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-4c-16b-rgba.tiff", dir);
    if (write_rgba(path, 16, "w") == 0)
        printf("  OK seq-4c-16b-rgba.tiff\n");
    else { printf("  FAIL seq-4c-16b-rgba.tiff\n"); ok = 0; }

    printf("\n--- Multi-page ---\n");

    snprintf(path, sizeof(path), "%s/seq-1c-8b-multipage.tiff", dir);
    if (write_multipage(path, 8, "w") == 0)
        printf("  OK seq-1c-8b-multipage.tiff\n");
    else { printf("  FAIL seq-1c-8b-multipage.tiff\n"); ok = 0; }

    printf("\n--- Big-endian ---\n");

    snprintf(path, sizeof(path), "%s/seq-1c-8b-bigendian.tiff", dir);
    if (write_gray_compressed(path, 8, COMPRESSION_NONE, PREDICTOR_NONE, "wb") == 0)
        printf("  OK seq-1c-8b-bigendian.tiff\n");
    else { printf("  FAIL seq-1c-8b-bigendian.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-1c-16b-bigendian.tiff", dir);
    if (write_gray_compressed(path, 16, COMPRESSION_NONE, PREDICTOR_NONE, "wb") == 0)
        printf("  OK seq-1c-16b-bigendian.tiff\n");
    else { printf("  FAIL seq-1c-16b-bigendian.tiff\n"); ok = 0; }

    snprintf(path, sizeof(path), "%s/seq-3c-8b-bigendian.tiff", dir);
    if (write_rgb_compressed(path, 8, COMPRESSION_NONE, "wb") == 0)
        printf("  OK seq-3c-8b-bigendian.tiff\n");
    else { printf("  FAIL seq-3c-8b-bigendian.tiff\n"); ok = 0; }

    printf("\nDone. %s\n", ok ? "All succeeded." : "Some failed!");
    return ok ? 0 : 1;
}
