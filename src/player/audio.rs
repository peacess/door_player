use std::sync::Arc;
use std::time::Duration;
use std::vec::IntoIter;

use cpal::SupportedStreamConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::Source;

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
    stream: cpal::Stream,
    config: SupportedStreamConfig,
    producer: ringbuf::Producer<f32, Arc<ringbuf::HeapRb<f32>>>,
}

impl AudioDevice {
    pub fn new() -> Result<Self, anyhow::Error> {
        let device = cpal::default_host().default_output_device().ok_or(ffmpeg::Error::OptionNotFound)?;

        let (mut producer,mut consumer) = ringbuf::HeapRb::<f32>::new(8192).split();

        let config = device.default_input_config()?;

        let stream = device.build_output_stream(&config.clone().into(),move |data: &mut [f32], cbinfo|{
            Self::write_audio(data, &mut consumer, cbinfo);
        }, |e| {
            log::error!("{}", e);
        }, None)?;

        Ok(Self {
            stream,
            config,
            producer,
        })
    }

    pub fn stream_config(&self) -> SupportedStreamConfig {
        self.config.clone()
    }

    pub fn play_source(&mut self, audio_frame: AudioFrame) {
        while self.producer.free_len() < audio_frame.samples.len() {
            spin_sleep::sleep(Duration::from_millis(10));
        }
        self.producer.push_slice(audio_frame.samples.as_slice());
    }

    pub fn set_mute(&self, mute: bool) {
        //todo
        // if mute {
        //     self.sink.set_volume(0.0);
        // } else {
        //     self.sink.set_volume(1.0);
        // }
    }

    pub fn set_pause(&self, pause: bool) {
        if pause {
            if let Err(e) = self.stream.pause(){
                log::error!("{}", e);
            }
        } else {
            if let Err(e) = self.stream.play(){
                log::error!("{}", e);
            }
        }
    }

    pub fn stop(&self) {
        if let Err(e) = self.stream.pause(){
            log::error!("{}", e);
        }
    }

    fn write_audio<T: cpal::Sample>(data: &mut [T], consumer: &mut ringbuf::Consumer<T,Arc<ringbuf::HeapRb<T>>>, _: &cpal::OutputCallbackInfo) {
        // if consumer.len() > 2 * 1024 * 1024 {
        //     consumer.pop_slice(data);
        // }else {
        //     for d in data.iter_mut() {
        //         *d = T::EQUILIBRIUM;
        //     }
        // }
        for d in data {
            // copy as many samples as we have.
            // if we run out, write silence
            match consumer.pop() {
                Some(sample) => *d = sample,
                None => *d = T::EQUILIBRIUM // Sample::from(&0.0)
            }
        }
    }
}

unsafe impl Send for AudioDevice {}

unsafe impl Sync for AudioDevice {}
