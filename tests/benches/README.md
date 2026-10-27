Copyrights:

kodim*.png: Eastman Kodak Company, released for unrestricted use
Transparency.png: Public Domain, according to Wikimedia

## Encoding benchmarks

Deflate uses Rust `miniz_oxide 0.9.1` through `flate2 1.1.10`, at level `1`.
Zstd uses native `libzstd 1.5.7` through `zstd 0.14.3`, at level `1`.
`0` selects the Zstd default, currently level `3`.
Both algorithms use RGB32Float data and floating-point prediction.
These settings do not guarantee equal file sizes.

The benchmark uses Criterion 0.7. On these photo benchmarks, interleaved comparisons with 0.8.2 showed no consistent timing or noise improvement.
Criterion 0.7 avoids adding a C compiler requirement to default test builds.

The benchmark prints each complete TIFF size in bytes with its full benchmark name before timing.

```sh
cargo bench --features zstd --bench encode
```

For a photo, set `TIFF_BENCH_DIR` to a directory containing
`photo-rgb-f32-deflate-fpred.tif`, a contiguous RGB32Float TIFF. The benchmark loads
the first image before timing, with a 1 GiB decode limit. A missing or incompatible file stops the run.
The input does not need a predictor. Use the same pixels for comparisons.

## Prepare the CC0 photos

Use LibRaw 0.22.2 (`dcraw_emu`) and ImageMagick 7.1.2-34 Q16-HDRI.
LibRaw provides [source and macOS/Windows binaries](https://www.libraw.org/download).
Put both commands on `PATH`. Run these POSIX shell commands from the repository root.
On Windows, use Git Bash.
On Linux and Windows, replace `shasum -a 256` below with `sha256sum`.
All downloads and generated images stay in the ignored `tests/benches/corpus/` directory.

Download the two [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) RAW files and check their SHA-256 hashes:

```sh
mkdir -p tests/benches/corpus/cc0/raw tests/benches/corpus/cc0/foliage tests/benches/corpus/cc0/city
curl -fL --retry 2 -o tests/benches/corpus/cc0/raw/P1000475.RW2 'https://raw.pixls.us/data/Panasonic/DC-G9/P1000475.RW2'
curl -fL --retry 2 -o tests/benches/corpus/cc0/raw/IMG_4059.CR2 'https://raw.pixls.us/data/Canon/PowerShot%20SX50%20HS/IMG_4059.CR2'
shasum -a 256 -c - <<'SHA256'
7c24fefb1f73ad45c42a0be0078e15b7e91deab1d623efceebfc3c4d0d06135c  tests/benches/corpus/cc0/raw/P1000475.RW2
6c2ddd232f5432fb7bd0e28efc12d66995cedcd847f801fd854bfdfb180db50c  tests/benches/corpus/cc0/raw/IMG_4059.CR2
SHA256
```

Develop 16-bit RGB PPM images with camera white balance and AHD demosaicing (`-q 3`).
Crop the foliage image to the center 6000 × 4000 pixels and the city image to the center 4000 × 3000 pixels.
Convert both to contiguous RGB32Float TIFFs. Neither image is resized.

```sh
dcraw_emu -w -q 3 -o 1 -6 -Z tests/benches/corpus/cc0/foliage/developed.ppm tests/benches/corpus/cc0/raw/P1000475.RW2
dcraw_emu -w -q 3 -o 1 -6 -Z tests/benches/corpus/cc0/city/developed.ppm tests/benches/corpus/cc0/raw/IMG_4059.CR2
magick tests/benches/corpus/cc0/foliage/developed.ppm -gravity center -crop 6000x4000+0+0 +repage -alpha off -interlace none -define quantum:format=floating-point -depth 32 -define tiff:predictor=floating-point -compress Zip tests/benches/corpus/cc0/foliage/photo-rgb-f32-deflate-fpred.tif
magick tests/benches/corpus/cc0/city/developed.ppm -gravity center -crop 4000x3000+0+0 +repage -alpha off -interlace none -define quantum:format=floating-point -depth 32 -define tiff:predictor=floating-point -compress Zip tests/benches/corpus/cc0/city/photo-rgb-f32-deflate-fpred.tif
magick identify -format '%wx%h\n' tests/benches/corpus/cc0/foliage/photo-rgb-f32-deflate-fpred.tif tests/benches/corpus/cc0/city/photo-rgb-f32-deflate-fpred.tif
```

The dimensions must be `6000x4000` and `4000x3000`.
Check the decoded pixel hashes below before comparing with the table.
These hashes cover RGB32Float samples in little-endian order, without TIFF metadata:

```sh
magick tests/benches/corpus/cc0/foliage/photo-rgb-f32-deflate-fpred.tif -define quantum:format=floating-point -depth 32 -endian LSB RGB:- | shasum -a 256
magick tests/benches/corpus/cc0/city/photo-rgb-f32-deflate-fpred.tif -define quantum:format=floating-point -depth 32 -endian LSB RGB:- | shasum -a 256
```

| Input | Decoded pixel SHA-256 |
| --- | --- |
| Foliage | `d6e57d713dabee6052556161118212f7bef2d8edf82e6ac0c353369d20965265` |
| City | `8f2f50909e98d32052c9727a8340b67b083ada6bff84fb1f6cbd935be0a06db9` |

Different tool versions or builds can change RAW development or float conversion. Use the same prepared pixels for both algorithms.

Run both photo comparisons twice:

```sh
for run in 1 2; do
  for photo in foliage city; do
    CARGO_PROFILE_BENCH_DEBUG=1 TIFF_BENCH_DIR="tests/benches/corpus/cc0/$photo" cargo bench --features zstd --bench encode -- photo-tiff-encode/ --warm-up-time 1 --measurement-time 3 --sample-size 20 --save-baseline "$photo-$run"
  done
done
```

Criterion saves each photo’s estimates under `target/criterion/photo-tiff-encode/<algorithm>/<photo>-<run>/estimates.json`.

## Photo measurements

Each iteration writes a new TIFF in memory. The benchmark decodes the source image before timing.
Both algorithms start with 4 bytes of output capacity per pixel. Timings include output allocation and buffer growth.
These results are from macOS arm64.
Each command collects 20 samples with a 1-second warm-up and a 3-second measurement target.

Large cases take longer to collect 20 samples.
Times are means across two process runs, in milliseconds and rounded to whole numbers.
Parentheses show the minimum and maximum run means.

| Input | Deflate 1 | Zstd 1 | Wall-time reduction |
| --- | ---: | ---: | ---: |
| 6000 × 4000 foliage photo | 809 (805–813) | 320 (318–322) | 60% |
| 4000 × 3000 city photo | 523 (519–527) | 142 (139–146) | 73% |

Complete TIFF sizes include tags and strip tables. One MiB is 1,048,576 bytes.

| Input | Deflate 1 (MiB) | Zstd 1 (MiB) | File-size reduction |
| --- | ---: | ---: | ---: |
| 6000 × 4000 foliage photo | 186.2 | 178.8 | 4.0% |
| 4000 × 3000 city photo | 85.9 | 82.7 | 3.7% |

Sources: CC0 [P1000475.RW2](https://raw.pixls.us/data/Panasonic/DC-G9/P1000475.RW2) and [IMG_4059.CR2](https://raw.pixls.us/data/Canon/PowerShot%20SX50%20HS/IMG_4059.CR2).
Both inputs are center crops. Neither is resized.

These results cover the two prepared photos and compression levels.
The measurements exclude disk writes.

The encoder allocates a compression context for each Zstd strip. Peak memory use was not measured.

To profile a prepared photo, enable debug symbols and attach a sampling profiler to the benchmark process:

```sh
CARGO_PROFILE_BENCH_DEBUG=1 TIFF_BENCH_DIR=tests/benches/corpus/cc0/foliage cargo bench --features zstd --bench encode -- photo-tiff-encode/f32-float-predictor-zstd-1 --profile-time 15
```
