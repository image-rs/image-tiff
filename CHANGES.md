# Version 0.7.0

New features:
* Support for encoding BigTiff ([#122](https://github.com/image-rs/image-tiff/pull/122))
  * _Breaking:_ Encoder types now have a generic parameter to differentiate BigTiff and standard Tiff encoding. Defaults to standard Tiff.
* Basic tile decoding ([#125](https://github.com/image-rs/image-tiff/pull/125))
  * _Breaking:_ There is a new `TiffError::UsageError` variant.
* Support for datatypes `Int8` and `Int16` ([#114](https://github.com/image-rs/image-tiff/pull/114))
  * _Breaking:_ `DecodingResult` and `DecodingBuffer` have the two new variants `I8` and `I16`.
* Support for `i32` arrays ([#118](https://github.com/image-rs/image-tiff/pull/118/files))
  * _Breaking:_ `DecodingResult` and `DecodingBuffer` have a new `I32` variant.
* Support for `Ifd` and `IfdBig` tag types and `I64` data type ([#119](https://github.com/image-rs/image-tiff/pull/119))
  * _Breaking:_ `DecodingResult` and `DecodingBuffer` have a new `I64` variant.
* Add `SMinSampleValue` and `SMaxSampleValue` ([#123](https://github.com/image-rs/image-tiff/pull/123))

Changes:
* Improve deflate support ([#132](https://github.com/image-rs/image-tiff/pull/132))
  * Switch to streaming decompression via `flate2`. Aside from performance improvements and lower RAM consumption, this fixes a bug where `max_uncompressed_length` was precalculated for a single tile but then used as a hard limit on the whole data, failing to decompress any tiled images.
  * Add support for new `Deflate` tag in addition to `OldDeflate`.
* _Breaking:_ Remove `InflateError`, which is no longer needed with `flate2` ([#134](https://github.com/image-rs/image-tiff/pull/134))
* _Breaking:_ Support for `MinIsWhite` is restricted to unsigned and floating
  point values. This is expected to be be re-added once some contradictory
  interpretation regarding semantics for signed values is resolved.

Fixes:
* Validate that ASCII tags are valid ASCII and end with a null byte ([#121](https://github.com/image-rs/image-tiff/pull/121))

Internal:
* Simplify decompression logic ([#126](https://github.com/image-rs/image-tiff/pull/126))
* Simplify `expand_strip` ([#128](https://github.com/image-rs/image-tiff/pull/128))

# Version 0.6.1

New features:
* Support for reading `u16` and ascii string tags.
* Added `Limits::unlimited` for disabling all limits.
* Added `ImageEncoder::rows_per_strip` to overwrite the default.

Changes:
* The default strip size for chunked encoding is now 1MB, up from 8KB. This
  should lead to more efficient decoding and compression.

Fixes:
* Fixed a bug where LZW compressed strips could not be decoded, instead
  returning an error `Inconsistent sizes encountered`.
* Reading a tag with a complex type and a single value returns the proper Value
  variant, instead of a vector with one entry.

# Version 0.6.0

New features:
* Support for decoding BigTIFF with 64-bit offsets
* The value types byte, `f32`, `f64` are now recognized
* Support for Modern JPEG encoded images

Improvements:
* Better support for adding auxiliary tags before encoding image data
* Switched to lzw decoder library `weezl` for performance
* The `ColorType` trait now supports `SAMPLE_ENCODING` hints

Fixes:
* Fixed decoding of inline ASCII in tags
* Fixed handling after null terminator in ASCII data
* Recognize tile and sample format tags

# Version 0.5.0

* Added support for 32-bit and 64-bit decoded values.
* Added CMYK(16|32|64) color type support.
* Check many internal integer conversions to increase stability. This should
  only lead to images being reported as faulty that would previously silently
  break platform limits. If there are any false positives, please report them.
* Remove an erroneous check of decoded length in lzw compressed images.

# Version 0.4.0

* Several enumerations are now non_exhaustive for future extensions.
  These are `Tag`, `Type`, `Value`, `PhotometricInterpretation`,
  `CompressionMethod`, `Predictor`.
* Enums gained a dedicated method to convert to their TIFF variant value with
  the specified type. Performing these conversions by casting the discriminant
  with `as` is not guaranteed to be stable, except where documented explicitly.
* Removed the num-derive and num dependencies.
* Added support for decoding `deflate` compressed images.
* Make the decoder `Limits` customizable by exposing members.
* Fixed multi-page TIFF encoding writing incorrect offsets.
