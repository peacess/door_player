use std::ffi::{c_void, CStr};
use std::sync::Arc;

use ffmpeg::{Rational, Rescale};
use ringbuf::HeapRb;

use crate::player::MILLISEC_TIME_BASE;

pub type RingBufferProducer<T> = ringbuf::Producer<T, Arc<HeapRb<T>>>;
pub type RingBufferConsumer<T> = ringbuf::Consumer<T, Arc<HeapRb<T>>>;

pub fn is_ffmpeg_eof_error(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ffmpeg::Error>(),
        Some(ffmpeg::Error::Eof)
    )
}

pub fn timestamp_to_millisecond(timestamp: i64, time_base: Rational) -> i64 {
    timestamp.rescale(time_base, MILLISEC_TIME_BASE)
}

pub struct FfmpegKit {}

impl FfmpegKit {
    pub fn demuxers() -> std::vec::Vec<String> {
        let mut names = std::vec::Vec::with_capacity(512);

        let mut opaque: *mut c_void = std::ptr::null_mut();
        let mut input_format: *const ffmpeg::ffi::AVInputFormat;
        unsafe {
            loop {
                input_format = ffmpeg::ffi::av_demuxer_iterate(&mut opaque as _);
                if input_format.is_null() {
                    break;
                }
                names.push(CStr::from_ptr((*input_format).name).to_str().expect("").to_string());
            }
        }
        names
    }
}


#[cfg(test)]
mod test {
    #[test]
    fn test_for() {
        let len = 2;
        for i in 1..len {
            println!("{}", i);
        }
    }
}