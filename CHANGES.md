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
