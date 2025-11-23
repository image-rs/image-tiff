use tiff::decoder::Decoder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(image) = std::env::args_os().nth(1) else {
        eprintln!("Usage: decode FILE");
        return Ok(());
    };

    let file = std::fs::File::open(image)?;
    let io = std::io::BufReader::new(file);
    let mut reader = Decoder::new(io)?;

    while {
        reader.read_image()?;
        reader.more_images()
    } {}

    Ok(())
}
