use std::sync::atomic::Ordering;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SupportedStreamConfig;

use crate::player::kits::RingBufferConsumer;

#[derive(Clone)]
pub struct AudioPlayFrame {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
    pub pts: i64,
    pub duration: i64,
}

impl AudioPlayFrame {
    pub fn new(samples: Vec<f32>, channels: u16, sample_rate: u32, pts: i64, duration: i64) -> Self {
        Self {
            samples,
            channels,
            sample_rate,
            pts,
            duration,
        }
    }
}

impl std::fmt::Debug for AudioPlayFrame {
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
    output_config: SupportedStreamConfig,
    mute: std::sync::atomic::AtomicBool,
}

impl AudioDevice {
    pub fn new<T: cpal::SizedSample + Send + 'static>(mut consumer: RingBufferConsumer<T>) -> Result<Self, anyhow::Error> {
        let device = cpal::default_host().default_output_device().ok_or(ffmpeg::Error::OptionNotFound)?;
        let output_config = {
            match device.default_output_config() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("{}", e);
                    // can not get the default config, then get the first supported config
                    let mut configs = device.supported_output_configs()?;
                    match configs.next() {
                        None => {
                            log::error!("No supported output config");
                            return Err(ffmpeg::Error::OptionNotFound.into());
                        }
                        Some(c) => c.with_max_sample_rate(),
                    }
                }
            }
        };
        let stream = device.build_output_stream(
            &output_config.clone().into(),
            move |data: &mut [T], info| {
                Self::write_audio(data, &mut consumer, info);
            },
            |e| {
                log::error!("{}", e);
            },
            None,
        )?;

        Ok(Self {
            stream,
            output_config,
            mute: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn output_config(&self) -> SupportedStreamConfig {
        self.output_config.clone()
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
        } else if let Err(e) = self.stream.play() {
            log::error!("{}", e);
        }
    }

    pub fn resume(&self) {
        self.set_pause(false);
    }
    pub fn pause(&self) {
        self.set_pause(true);
    }

    fn write_audio<T: cpal::Sample>(data: &mut [T], consumer: &mut RingBufferConsumer<T>, _: &cpal::OutputCallbackInfo) {
        if !consumer.is_empty() {
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
