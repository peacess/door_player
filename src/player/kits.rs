use std::{fs, path};
use std::collections::HashSet;
use std::ffi::{c_void, CStr};
use std::path::PathBuf;
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
    pub fn demuxers() -> Vec<String> {
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
                    let ns: Vec<_> = name.split(',').map(|s| s.to_string()).collect();
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
        names = HashSet::<String>::from_iter(names).into_iter().collect::<Vec<String>>();
        names.sort();
        log::info!("all file type: {:?}", names);
        names
    }

    // pub fn hwaccels() -> Vec<String> {
    //
    // }
}

pub struct Volume {}

impl Volume {
    pub const MAX_F64_VOLUME: f64 = 1.0;
    pub const MIN_F64_VOLUME: f64 = 0.0;
    pub const DEFAULT_F64_VOLUME: f64 = 0.5;

    pub const MAX_INT_VOLUME: i64 = 1000;

    pub const MIN_INT_VOLUME: i64 = 0;
    pub const VOLUME_STEP: i64 = 5;
    pub fn plus_volume(volume: f64) -> f64 {
        let mut v = (volume * Self::MAX_INT_VOLUME as f64) as i64 + Self::VOLUME_STEP;
        if v > Self::MAX_INT_VOLUME {
            v = Self::MAX_INT_VOLUME;
        }
        v as f64 / Self::MAX_INT_VOLUME as f64
    }

    pub fn minus_volume(volume: f64) -> f64 {
        let mut v = (volume * Self::MAX_INT_VOLUME as f64) as i64 - Self::VOLUME_STEP;
        if v < Self::MIN_INT_VOLUME {
            v = Self::MIN_INT_VOLUME;
        }
        v as f64 / Self::MAX_INT_VOLUME as f64
    }

    pub fn int_volume(volume: f64) -> i64 {
        (volume * Self::MAX_INT_VOLUME as f64) as i64
    }
    pub fn f64_volume(int_volume: i64) -> f64 {
        int_volume as f64 / Self::MAX_INT_VOLUME as f64
    }
}

pub struct SubTitle {}

impl SubTitle {
    /// sub title extension: srt,ass,ssa,sub,smi
    pub fn sub_files(file: &str) -> Vec<PathBuf> {
        let mut subs = Vec::with_capacity(6);
        let path_file = path::PathBuf::from(file);
        if path_file.file_name().is_none() {
            return subs;
        }
        match fs::read_dir(path_file.parent().unwrap()) {
            Err(e) => {
                log::error!("{}",e);
                return subs;
            }
            Ok(read_dir) => {
                let no_ex = path_file.file_stem().expect("").to_str().expect("").to_string();
                let file_name = path_file.file_name().expect("");
                let exs = ["srt", "ass", "ssa", "sub", "smi"]; // array is better than mam/set
                for ff in read_dir.flatten() {
                    let n = ff.file_name().to_str().expect("").to_string();
                    if file_name != ff.file_name() && n.starts_with(&no_ex) {
                        if let Some(ex_name) = ff.path().extension() {
                            let t = ex_name.to_str().expect("");
                            if exs.contains(&t) {
                                subs.push(ff.path());
                            }
                        }
                    }
                }
            }
        }

        subs.sort();
        subs
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