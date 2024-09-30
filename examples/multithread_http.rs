// Special thanks to Alice for the help: https://users.rust-lang.org/t/63019/6
use std::io::{Result, SeekFrom};
use std::pin::Pin;
use std::sync::Arc;
use futures::{
    future::BoxFuture,
    io::{AsyncRead, AsyncSeek},
    Future,
};
use tiff::decoder::Decoder;

// extern crate ehttp;

// Arc for sharing, see https://users.rust-lang.org/t/how-to-clone-a-boxed-closure/31035/9
// or https://stackoverflow.com/a/27883612/14681457
pub type F = Arc<
    dyn Fn(u64, u64) -> BoxFuture<'static, std::io::Result<SeekOutput>> + Send + Sync,
>;
pub struct RangedStreamer {
    pos: u64,
    length: u64, // total size
    state: State,
    range_get: F,
    min_request_size: usize, // requests have at least this size
}

/// This is a fake clone, that doesn't clone the currently pending task, but everything else
impl Clone for RangedStreamer {
    fn clone(&self) -> Self {
        RangedStreamer {
            pos: self.pos,
            length: self.length,
            state: State::HasChunk(SeekOutput {
                start: 0,
                data: vec![],
            }),
            range_get: self.range_get.clone(),
            min_request_size: self.min_request_size,
        }
    }
}

enum State {
    HasChunk(SeekOutput),
    Seeking(BoxFuture<'static, std::io::Result<SeekOutput>>),
}

#[derive(Debug, Clone)]
pub struct SeekOutput {
    pub start: u64,
    pub data: Vec<u8>,
}



impl RangedStreamer {
    pub fn new(length: usize, min_request_size: usize, range_get: F) -> Self {
        let length = length as u64;
        Self {
            pos: 0,
            length,
            state: State::HasChunk(SeekOutput {
                start: 0,
                data: vec![],
            }),
            range_get,
            min_request_size,
        }
    }
}

// whether `test_interval` is inside `a` (start, length).
fn range_includes(a: (usize, usize), test_interval: (usize, usize)) -> bool {
    if test_interval.0 < a.0 {
        return false;
    }
    let test_end = test_interval.0 + test_interval.1;
    let a_end = a.0 + a.1;
    if test_end > a_end {
        return false;
    }
    true
}

impl AsyncRead for RangedStreamer {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<Result<usize>> {
        let requested_range = (self.pos as usize, buf.len());
        let min_request_size = self.min_request_size;
        match &mut self.state {
            State::HasChunk(output) => {
                let existing_range = (output.start as usize, output.data.len());
                if range_includes(existing_range, requested_range) {
                    let offset = requested_range.0 - existing_range.0;
                    buf.copy_from_slice(&output.data[offset..offset + buf.len()]);
                    self.pos += buf.len() as u64;
                    std::task::Poll::Ready(Ok(buf.len()))
                } else {
                    let start = requested_range.0 as u64;
                    let length = std::cmp::max(min_request_size, requested_range.1);
                    let future = (self.range_get)(start, length.try_into().unwrap());
                    self.state = State::Seeking(Box::pin(future));
                    self.poll_read(cx, buf)
                }
            }
            State::Seeking(ref mut future) => match Pin::new(future).poll(cx) {
                std::task::Poll::Ready(v) => {
                    match v {
                        Ok(output) => self.state = State::HasChunk(output),
                        Err(e) => return std::task::Poll::Ready(Err(e)),
                    };
                    self.poll_read(cx, buf)
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            },
        }
    }
}

impl AsyncSeek for RangedStreamer {
    fn poll_seek(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        pos: std::io::SeekFrom,
    ) -> std::task::Poll<Result<u64>> {
        match pos {
            SeekFrom::Start(pos) => self.pos = pos,
            SeekFrom::End(pos) => self.pos = (self.length as i64 + pos) as u64,
            SeekFrom::Current(pos) => self.pos = (self.pos as i64 + pos) as u64,
        };
        std::task::Poll::Ready(Ok(self.pos))
    }
}



#[tokio::main]
async fn main() {
    let url = "https://isdasoil.s3.amazonaws.com/covariates/dem_30m/dem_30m.tif";
    let Ok(url_head) = ehttp::fetch_async(ehttp::Request::head(url)).await else {println!("EPIC FAIL!"); return;};
    let length = usize::from_str_radix(url_head.headers.get("content-length").unwrap(), 10).expect("Could not parse content length");
    println!("head: {:?}", url_head);
    let range_get = Arc::new(move |start: u64, length: u64| {
        // let bucket = bucket.clone();
        let url = url;
        Box::pin(async move {
            println!("requested: {} kb", length / 1024);
            let mut request = ehttp::Request::get(url);
            request.headers.insert("Range".to_string(), format!("bytes={:?}-{:?}",start,start+length));
            let resp = ehttp::fetch_async(request).await.map_err(|e| std::io::Error::other(e))?;
            if !resp.ok {
                Err(std::io::Error::other(format!("Received invalid response: {:?}", resp.status)))
            } else {
                println!("received: {} kb", resp.bytes.len() / 1024);
                Ok(SeekOutput {start, data: resp.bytes}) 
            }
        }) as BoxFuture<'static, std::io::Result<SeekOutput>>
    });
    let reader = RangedStreamer::new(length, 30*1024, range_get);
    
    // this decoder will read all necessary tags
    let decoder =  Decoder::new_overview_async(reader, 0).await.expect("oh noes!");
    println!("initialized decoder");
    let cloneable_decoder = tiff::decoder::ChunkDecoder::from_decoder(decoder);
    
    let mut handles = Vec::new();
    for chunk in 42..69 {
        let mut cloned_decoder = cloneable_decoder.clone();

        let handle = tokio::spawn(async move {
            
            let result = cloned_decoder.read_chunk_async(chunk).await;
            match result {
                Ok(data) => {
                    println!("Successfully read chunk {}", chunk);
                    Ok(data)  // Return the data for collection
                }
                Err(e) => {
                    eprintln!("Error reading chunk {}: {:?}", chunk, e);
                    Err(e)  // Return the error for handling
                }
            }
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    for r in results {
        println!("result: {:?}", r.expect("idk").expect("idk²").len())
    }
}
