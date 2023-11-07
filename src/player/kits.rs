use std::collections::HashSet;
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
                if !(*input_format).name.is_null() {
                    let name = CStr::from_ptr((*input_format).name).to_str().expect("");
                    let ns: Vec<_> = name.split(",").map(|s| s.to_string()).collect();
                    names.extend(ns);
                }
                // {
                //     if  !(*input_format).long_name.is_null() {
                //         log::info!("long_name: {}", CStr::from_ptr((*input_format).long_name).to_str().expect(""));
                //     }
                //     log::info!("name: {}, flags: {}", CStr::from_ptr((*input_format).name).to_str().expect(""),
                //         (*input_format).flags
                //     );
                //     if !(*input_format).extensions.is_null() {
                //         log::info!("extensions: {}", CStr::from_ptr((*input_format).extensions).to_str().expect(""));
                //     }
                //     if !(*input_format).mime_type.is_null() {
                //         log::info!("mime_type: {}", CStr::from_ptr((*input_format).mime_type).to_str().expect(""));
                //     }
                //     // log::info!("raw_codec_id: {}, priv_data_size {}\n", (*input_format).raw_codec_id, (*input_format).raw_codec_id);
                //     log::info!("\n{:?}\n", &*input_format);
                //
                // }
            }
        }
        if !names.contains(&"mkv".to_string()) {
            names.push("mkv".to_string());
        }
        names = HashSet::<String>::from_iter(names.into_iter()).into_iter().collect::<Vec<String>>();
        names.sort();
        log::info!("all file type: {:?}", names);
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