# Making Tiff crate async for Cogs

This folder is a bunch of copied code, made async with futures crate. This makes the reader runtime agnostic, as well as backend agnostic. This allows it to be used anywhere (tokio, smol, bevy), from any datasource that provides an AsyncReader. For using this on Sync stuff, such as files, use the following snippet:

```rust
use futures::io::AllowStdIo;
use std::fs::File;
use tiff::decoder_async::Decoder;

#[tokio::main]
fn main() {
    let f = AllowStdIo(File("/path/to/file.tiff"));
    let mut decoder = Decoder::new(f).await.expect("Could not open file");
    let result = decoder.read_image();
}
```

For more fine-grained control over what the tiff does, another method, `new_raw` is provided, that does not read all tags by default, since this caused excessive requests when testing on isdasoil data.

The plan is:

1. Read image and check header
2. Scan all IFD tags to see how many overviews there are
   - may actually read into values that don't hold pointers, since we're ideally buffering anyways. ChatGPT says all metadata (excluding tile offsets) should be in the first ~16kB for COGs, which is a - 16 * 1024 byte buffer, which is not that big.
   -  The call chain that slows execution is: `decoder.next_image()->tag_reader.find_tag(tag)->entry.clone().val()->case 4 entry.decode_offset()` where `decode_offset` possibly goes out of the currently read buffer. Of course, this could be circumvented by having a grow-only buffer, but then the reader and decoder would have to be more tightly coupled
     - another alternative (which I kind of like) is to (optionally) store offset data in our own buffer that is the right size. However, for the largest overview, even this may be kind of big?
     - ideally, we would be able to pass as arguments to our reader: read_bytes(a, b), because that would directly translate to a range request. <- this could be a homebrew trait with a blanket implementation?
3. Read only (the first?) IFD tag
4. Load tiles on-demand


I think the current implementation is mainly inefficient because tags that hold more data than what fits in an IFD entries' Value field, the Value is a pointer that gets followed through (because a file was assumed). 
