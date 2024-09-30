// Special thanks to Alice for the help: https://users.rust-lang.org/t/63019/6
use std::io::{Error, Result, SeekFrom};
use std::pin::Pin;

use futures::{
    future::BoxFuture,
    io::{AsyncRead, AsyncSeek},
    Future,
};
use tiff::decoder::ifd::Value;
use tiff::decoder::DecodingResult;
use tiff::decoder::Decoder;

// extern crate ehttp;


pub struct RangedStreamer {
    state: State,
    range_get: F,
    min_request_size: usize, // requests have at least this size
}

enum State {
    HasChunk(SeekOutput),
    Seeking(BoxFuture<'static, std::io::Result<SeekOutput>>),
}

pub struct SeekOutput {
    pub start: u64,
    pub data: Vec<u8>,
}

pub type F = Box<
    dyn Fn(u64, u64) -> BoxFuture<'static, std::io::Result<SeekOutput>> + Send + Sync,
>;

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
    let range_get = Box::new(move |start: u64, length: u64| {
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
    match Decoder::new_overview_async(reader, 5).await {
        Ok(mut d) => println!("{:?}", d.read_chunk_async(42).await.unwrap()),
        Err(e) => println!("err: {:?}", e),
    }
}
