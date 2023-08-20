# Examples

Below are some examples of integrating image-tiff with other libraries for additional functionality. 

## CCITT Group 4 / T.6 "fax machine" compression support

[This example](group_4) uses the [fax crate](https://github.com/pdf-rs/fax) to provide CCITT Group 4 / T.6 decompression capabilities to 
the image-tiff crate in a simple demo that accepts an arbitrary image file as the sole command line argument, then performs
group 4 decompression on the input file if it is a big endian tiff file and writes it out as a `.png` file. Sample files are included 
under the data folder.