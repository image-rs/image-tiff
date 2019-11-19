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
