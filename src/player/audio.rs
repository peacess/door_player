use std::time::Duration;
use std::vec::IntoIter;

use cpal::{FromSample, SupportedStreamConfig};
use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{OutputStream, Sample, Sink, Source};

use crate::player::player::PlayFrame;

#[derive(Clone)]
pub struct AudioFrame {
    pub samples: IntoIter<f32>,
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
            samples: samples.into_iter(),
            channels,
            sample_rate,
            pts,
            duration,
        }
    }
}

impl PlayFrame for AudioFrame {
    fn pts(&self) -> f64 {
        self.pts
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn mem_size(&self) -> usize {
        // std::mem::size_of::<Self>() +
        std::mem::size_of::<f32>() * self.samples.len()
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

impl Iterator for AudioFrame {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.samples.next()
    }
}

impl Source for AudioFrame {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.samples.len())
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        Some(Duration::from_secs_f64(self.duration))
    }
}

pub struct AudioDevice {
    _stream: OutputStream,
    sink: Sink,
    default_config: SupportedStreamConfig,
}

impl AudioDevice {
    pub fn new() -> Result<Self, anyhow::Error> {
        let default_device = cpal::default_host().default_output_device().ok_or(ffmpeg::Error::OptionNotFound)?;

        let default_config = default_device.default_input_config()?;

        let default_stream = OutputStream::try_from_device(&default_device);

        let (_stream, handle) = default_stream
            .or_else(|original_err| {
                // default device didn't work, try other ones
                let mut devices = match cpal::default_host().output_devices() {
                    Ok(d) => d,
                    Err(_) => return Err(original_err),
                };
                devices
                    .find_map(|d| OutputStream::try_from_device(&d).ok())
                    .ok_or(original_err)
            })?;

        let sink = rodio::Sink::try_new(&handle).unwrap();
        Ok(Self {
            _stream,
            sink,
            default_config,
        })
    }

    pub fn default_config(&self) -> SupportedStreamConfig {
        self.default_config.clone()
    }

    pub fn play_source<S>(&self, audio_source: S)
        where
            S: Source + Send + 'static,
            f32: FromSample<S::Item>,
            S::Item: Sample + Send,
    {
        self.sink.append(audio_source);
    }

    pub fn set_mute(&self, mute: bool) {
        if mute {
            self.sink.set_volume(0.0);
        } else {
            self.sink.set_volume(1.0);
        }
    }

    pub fn set_pause(&self, pause: bool) {
        if pause {
            self.sink.pause();
        } else {
            self.sink.play();
        }
    }

    pub fn stop(&self) {
        self.sink.stop();
    }
}

unsafe impl Send for AudioDevice {}

unsafe impl Sync for AudioDevice {}
