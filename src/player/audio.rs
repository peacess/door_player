use std::sync::atomic::Ordering;

use cpal::SupportedStreamConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::player::RingBufferConsumer;

#[derive(Clone)]
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
    pub pts: f64,
    pub duration: f64,
}

impl AudioFrame {
    pub fn new(
        samples: Vec<f32>,
        channels: u16,
        sample_rate: u32,
        pts: f64,
        duration: f64,
    ) -> Self {
        Self {
            samples,
            channels,
            sample_rate,
            pts,
            duration,
        }
    }
}

impl std::fmt::Debug for AudioFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioFrame")
            // .field("samples len", &self.samples.len())
            // .field("channels", &self.channels)
            // .field("sample_rate", &self.sample_rate)
            .field("pts", &self.pts)
            .field("duration", &self.duration)
            .finish()
    }
}

pub struct AudioDevice {
    stream: cpal::Stream,
    config: SupportedStreamConfig,
    mute: std::sync::atomic::AtomicBool,
}

impl AudioDevice {
    pub fn new<T: cpal::SizedSample + Send + 'static>(mut consumer: RingBufferConsumer<T>) -> Result<Self, anyhow::Error> {
        let device = cpal::default_host().default_output_device().ok_or(ffmpeg::Error::OptionNotFound)?;
        let config = device.default_input_config()?;
        let stream = device.build_output_stream(&config.clone().into(), move |data: &mut [T], cbinfo| {
            Self::write_audio(data, &mut consumer, cbinfo);
        }, |e| {
            log::error!("{}", e);
        }, None)?;

        Ok(Self {
            stream,
            config,
            mute: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn stream_input_config(&self) -> SupportedStreamConfig {
        self.config.clone()
    }

    pub fn set_mute(&self, mute: bool) {
        self.mute.store(mute, Ordering::Relaxed);
    }

    pub fn get_mute(&self) -> bool {
        self.mute.load(Ordering::Relaxed)
    }

    fn set_pause(&self, pause: bool) {
        if pause {
            if let Err(e) = self.stream.pause() {
                log::error!("{}", e);
            }
        } else {
            if let Err(e) = self.stream.play() {
                log::error!("{}", e);
            }
        }
    }

    pub fn resume(&self) {
        self.set_pause(false);
    }
    pub fn pause(&self) {
        self.set_pause(true);
    }

    fn write_audio<T: cpal::Sample>(data: &mut [T], consumer: &mut RingBufferConsumer<T>, _: &cpal::OutputCallbackInfo) {
        if consumer.len() > 0 {
            let done = consumer.pop_slice(data);
            if done < data.len() {
                let s = &mut data[done..];
                s.fill(T::EQUILIBRIUM);
            }
        } else {
            data.fill(T::EQUILIBRIUM);
        }
        // for d in data {
        //     // copy as many samples as we have.
        //     // if we run out, write silence
        //     match consumer.pop() {
        //         Some(sample) => *d = sample,
        //         None => *d = T::EQUILIBRIUM // Sample::from(&0.0)
        //     }
        // }
    }
}

unsafe impl Send for AudioDevice {}

unsafe impl Sync for AudioDevice {}
