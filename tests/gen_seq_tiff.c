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

    printf("\nDone. %s\n", ok ? "All succeeded." : "Some failed!");
    return ok ? 0 : 1;
}
