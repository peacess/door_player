use std::ops::{Deref, DerefMut};
use std::path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::{Image, Response, Sense, Ui};
use egui::load::SizedTexture;
use ffmpeg::Rational;
use ffmpeg::software::resampling::Context as ResamplingContext;
use kanal::{Receiver, Sender};
use parking_lot::Mutex;

use crate::player::audio::{AudioDevice, AudioFrame};
use crate::player::consts::{AUDIO_FRAME_QUEUE_SIZE, AUDIO_PACKET_QUEUE_SIZE, PLAY_MIN_INTERVAL, VIDEO_FRAME_QUEUE_SIZE, VIDEO_PACKET_QUEUE_SIZE};
use crate::player::play_ctrl::PlayCtrl;
use crate::player::video::VideoFrame;
use crate::PlayerState;

// use ffmpeg::format::Sample;

pub trait PlayFrame: std::fmt::Debug {
    fn pts(&self) -> f64 {
        0.0
    }
    fn duration(&self) -> f64 {
        0.0
    }
    fn mem_size(&self) -> usize {
        0
    }
}

/// player base ffmpeg, there are 4 threads to player file.
pub struct Player {
    file_path: String,
    //是否需要停止播放相关线程
    pub play_ctrl: PlayCtrl,
    pub width: u32,
    pub height: u32,
}

impl Player {
    //初始化所有线程，如果之前的还在，结束它们
    pub fn new(ctx: &egui::Context, file: &str) -> Result<Player, anyhow::Error> {
        let abort_req = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let play_ctrl = {
            let audio_dev = AudioDevice::new()?;
            let audio_dev = Arc::new(Mutex::new(audio_dev));
            audio_dev.lock().set_pause(false);

            PlayCtrl::new(audio_dev, abort_req, Self::default_texture_handle(ctx))
        };
        let mut player = Self {
            file_path: file.to_string(),
            play_ctrl,
            width: 0,
            height: 0,
        };
        //打开文件
        let format_input = ffmpeg::format::input(&path::Path::new(file))?;

        // 获取视频解码器
        let (video_index, video_decoder) = {
            let video_stream = format_input.streams().best(ffmpeg::media::Type::Video).ok_or(ffmpeg::Error::InvalidData)?;
            let video_index = video_stream.index();
            let video_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
            let video_decoder = video_context.decoder().video()?;
            player.width = video_decoder.width();
            player.height = video_decoder.height();
            (video_index, video_decoder)
        };

        // 获取音频解码器
        let (audio_index, audio_decoder) = {
            let audio_stream = format_input.streams().best(ffmpeg::media::Type::Audio).ok_or(ffmpeg::Error::InvalidData)?;
            let audio_index = audio_stream.index();
            let audio_context = ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())?;
            let audio_decoder = audio_context.decoder().audio()?;
            (audio_index, audio_decoder)
        };

        let (audio_packet_sender, audio_packet_receiver) = kanal::bounded(AUDIO_PACKET_QUEUE_SIZE);
        let (video_packet_sender, video_packet_receiver) = kanal::bounded(VIDEO_PACKET_QUEUE_SIZE);

        let (audio_frame_tx, audio_frame_rx) = kanal::bounded::<AudioFrame>(AUDIO_FRAME_QUEUE_SIZE);
        let (video_frame_tx, video_frame_rx) = kanal::bounded::<VideoFrame>(VIDEO_FRAME_QUEUE_SIZE);

        let video_time_base = format_input.stream(video_index).expect("").time_base();
        // .ok_or_else(|| PlayerError::Error(format!("根据 stream_idx 无法获取到 stream")))?
        // .time_base;

        //开启 音频解码线程
        player.audio_decode_run(audio_decoder, audio_packet_receiver, audio_frame_tx);
        //开启 音频播放线程
        player.audio_play_run(audio_frame_rx);
        //开启 视频解码线程
        player.video_decode_run(video_decoder, video_packet_receiver, video_frame_tx, video_time_base);
        //开启 视频播放
        player.video_play_run(ctx.clone(), video_frame_rx);
        //开启 读frame线程
        player.read_packet_run(format_input, audio_packet_sender, audio_index,
                               video_packet_sender, video_index);

        // player.play_ctrl.set_pause(false);
        Ok(player)
    }

    pub fn default_texture_handle(ctx: &egui::Context) -> egui::TextureHandle {
        let texture_options = egui::TextureOptions::LINEAR;
        let texture_handle = ctx.load_texture("video_stream_default", egui::ColorImage::example(), texture_options);
        texture_handle
    }

    pub fn frame_to_color_image(frame: &ffmpeg::util::frame::Video) -> Result<egui::ColorImage, ffmpeg::Error> {
        let mut rgb_frame = ffmpeg::util::frame::Video::empty();
        let mut context = ffmpeg::software::scaling::Context::get(
            frame.format(),
            frame.width(),
            frame.height() as u32,
            ffmpeg::format::Pixel::RGB24,
            frame.width(),
            frame.height() as u32,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?;
        context.run(frame, &mut rgb_frame)?;

        let size = [rgb_frame.width() as usize, rgb_frame.height() as usize];
        let data = rgb_frame.data(0);
        let stride = rgb_frame.stride(0);
        let pixel_size_bytes = 3;
        let byte_width: usize = pixel_size_bytes * rgb_frame.width() as usize;
        let height: usize = rgb_frame.height() as usize;
        let mut pixels = vec![];
        for line in 0..height {
            let begin = line * stride;
            let end = begin + byte_width;
            let data_line = &data[begin..end];
            pixels.extend(
                data_line
                    .chunks_exact(pixel_size_bytes)
                    .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2])),
            )
        }
        Ok(egui::ColorImage { size, pixels })
    }

    fn audio_decode_run(&self, mut audio_decoder: ffmpeg::decoder::Audio, packet_receiver: Receiver<ffmpeg::Packet>, audio_deque: Sender<AudioFrame>) {
        let play_ctrl = self.play_ctrl.clone();
        let mut audio_re_sampler = {
            let stream_config = play_ctrl.audio_default_config();
            match ResamplingContext::get(
                audio_decoder.format(),
                audio_decoder.channel_layout(),
                audio_decoder.rate(),
                to_sample(stream_config.sample_format()),
                audio_decoder.channel_layout(),
                stream_config.sample_rate().0,
            ) {
                Err(e) => {
                    log::error!("{}", e);
                    panic!("{}", e);
                }
                Ok(t) => t
            }
        };
        std::thread::spawn(move || {
            'RUN: loop {
                if play_ctrl.abort_req() {
                    break 'RUN;
                }

                loop {
                    let mut frame_old = unsafe { ffmpeg::frame::Audio::empty() };
                    match audio_decoder.receive_frame(&mut frame_old) {
                        Ok(_) => {
                            let mut frame_resample = ffmpeg::frame::Audio::empty();
                            match audio_re_sampler.run(&frame_old, &mut frame_resample) {
                                Err(e) => {
                                    log::error!("{}", e);
                                    continue;
                                }
                                Ok(_) => {
                                    //todo delay
                                }
                            }
                            let re_samples_ref = if frame_resample.is_packed() {
                                Self::packed(&frame_resample)
                            } else {
                                frame_resample.plane(0)
                            };
                            let pts = frame_old.pts().expect("") as f64 / frame_old.rate() as f64;
                            let duration = frame_old.samples() as f64 / frame_old.rate() as f64;
                            let v = play_ctrl.volume();
                            let samples: Vec<f32> = re_samples_ref.iter().map(|s| s * v).collect();
                            let audio_frame = AudioFrame {
                                samples: samples.into_iter(),
                                channels: frame_resample.channels(),
                                sample_rate: frame_resample.rate(),
                                pts,
                                duration,
                            };
                            match audio_deque.send(audio_frame) {
                                Err(e) => {
                                    log::error!("{}", e);
                                }
                                Ok(_) => {}
                            }
                        }
                        Err(e) => {
                            log::info!("{}", e);
                            break;
                        }
                    }
                    if play_ctrl.abort_req() {
                        break 'RUN;
                    }
                }

                match packet_receiver.recv() {
                    Err(e) => {
                        log::error!("{}", e);
                        break 'RUN;
                    }
                    Ok(packet) => {
                        if play_ctrl.abort_req() {
                            break 'RUN;
                        }
                        match audio_decoder.0.send_packet(&packet) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                    }
                }
            }
        });
    }

    fn audio_play_run(&self, frame_deque: Receiver<AudioFrame>) {
        let mut play_ctrl = self.play_ctrl.clone();
        std::thread::spawn(move || {
            let mut empty_count = 0;
            loop {
                if play_ctrl.abort_req() {
                    break;
                }
                match frame_deque.try_recv() {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(None) => {}
                    Ok(Some(frame)) => {
                        match play_ctrl.play_audio(frame) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                        empty_count = 0;
                        continue;
                    }
                }

                empty_count += 1;
                if empty_count == 10 {
                    if play_ctrl.pause() {
                        empty_count = 0;
                    } else {
                        play_ctrl.set_audio_finished(true);
                        break;
                    }
                }
                spin_sleep::sleep(PLAY_MIN_INTERVAL);
            }
        });
    }

    fn video_decode_run(&self, mut video_decoder: ffmpeg::decoder::Video, packet_receiver: kanal::Receiver<ffmpeg::Packet>, video_deque: Sender<VideoFrame>, video_time_base: Rational) {
        let play_ctrl = self.play_ctrl.clone();
        let width = video_decoder.width() as usize;
        let height = video_decoder.height() as usize;

        // let duration = 1.0 / av_q2d(video_decoder..framerate);

        std::thread::spawn(move || {
            loop {
                if play_ctrl.abort_req() {
                    break;
                }
                let mut frame = unsafe { ffmpeg::util::frame::Video::empty() };
                match video_decoder.receive_frame(&mut frame) {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(_) => {
                        let color_image = match Self::frame_to_color_image(&frame) {
                            Err(e) => {
                                log::error!("{}", e);
                                continue;
                            }
                            Ok(t) => t,
                        };
                        let pts = frame.pts().unwrap_or_default() as f64;

                        let duration = {
                            match video_decoder.frame_rate() {
                                None => {
                                    log::error!("the frame_rate is null");
                                    return;
                                }
                                Some(t) => {
                                    1.0 / f64::from(t)
                                }
                            }
                        };

                        let video_frame = VideoFrame {
                            width,
                            height,
                            pts,
                            duration,
                            color_image,
                        };
                        match video_deque.send(video_frame) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                    }
                }
                match packet_receiver.recv() {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(packet) => {
                        match video_decoder.0.send_packet(&packet) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                    }
                }
            }
        });
    }

    fn video_play_run(&self, ctx: egui::Context, mut frame_deque: Receiver<VideoFrame>) {
        let mut play_ctrl = self.play_ctrl.clone();
        std::thread::spawn(move || {
            let mut empty_count = 0;
            loop {
                if play_ctrl.abort_req() {
                    break;
                }
                match frame_deque.try_recv() {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(None) => {}
                    Ok(Some(frame)) => {
                        play_ctrl.play_video(frame, &ctx)?;
                        empty_count = 0;
                        continue;
                    }
                }

                empty_count += 1;
                if empty_count == 10 {
                    if play_ctrl.pause() {
                        empty_count = 0;
                    } else {
                        play_ctrl.set_video_finished(true);
                        break;
                    }
                }
                spin_sleep::sleep(PLAY_MIN_INTERVAL);
            }

            Ok::<(), anyhow::Error>(())
        });
    }

    fn read_packet_run(&self, mut input: ffmpeg::format::context::Input, audio_deque: kanal::Sender<ffmpeg::Packet>, audio_index: usize,
                       video_deque: kanal::Sender<ffmpeg::Packet>, video_index: usize) {
        let mut play_ctrl = self.play_ctrl.clone();
        let duration = input.duration();
        std::thread::spawn(move || {
            loop {
                if play_ctrl.abort_req() {
                    break;
                }

                if play_ctrl.audio_finished() && play_ctrl.video_finished() {
                    play_ctrl.set_abort_req(true);
                    break;
                }

                let scale = play_ctrl.seek_scale.load(Ordering::Relaxed);
                if scale > 0.0 {
                    let seek_pos = (scale * duration as f64) as i64;
                    if let Err(e) = input.seek(seek_pos, ..seek_pos) {
                        log::error!("{}", e);
                    }
                    play_ctrl.seek(-1.0);
                } else {
                    if play_ctrl.pause() || audio_deque.is_full() || video_deque.is_full() {
                        spin_sleep::sleep(PLAY_MIN_INTERVAL);
                        continue;
                    }
                }

                if let Some((s, packet)) = input.packets().next() {
                    if unsafe { !packet.is_empty() } {
                        if packet.stream() == audio_index {
                            if let Err(e) = audio_deque.send(packet) {
                                log::error!("{}", e);
                            }
                        } else if packet.stream() == video_index {
                            if let Err(e) = video_deque.send(packet) {
                                log::error!("{}", e);
                            }
                        }
                    }
                } else {
                    play_ctrl.set_demux_finished(true);
                    spin_sleep::sleep(PLAY_MIN_INTERVAL);
                }
            }
        });
    }

    pub fn packed<T: ffmpeg::frame::audio::Sample>(frame: &ffmpeg::frame::Audio) -> &[T] {
        if !frame.is_packed() {
            panic!("data is not packed");
        }

        if !<T as ffmpeg::frame::audio::Sample>::is_valid(frame.format(), frame.channels()) {
            panic!("unsupported type");
        }

        unsafe { std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const T, frame.samples() * frame.channels() as usize) }
    }
}

impl Player {
    pub fn ui(&mut self, ui: &mut Ui, size: [f32; 2]) -> Response {
        let image = Image::new(SizedTexture::new(self.play_ctrl.texture_handle.id(), size)).sense(Sense::click());
        let response = ui.add(image);
        // self.render_ui(ui, &response);
        // self.process_state();
        response
    }
    pub fn start(&mut self) {
        self.resume();
    }
    pub fn pause(&mut self) {
        self.set_state(PlayerState::Paused);
    }
    pub fn resume(&mut self) {
        self.set_state(PlayerState::Playing);
    }
    pub fn stop(&mut self) {
        self.set_state(PlayerState::Stopped);
    }
    // seek in play ctrl
    fn set_state(&mut self, new_state: PlayerState) {
        self.player_state.set(new_state);
    }
}

impl Deref for Player {
    type Target = PlayCtrl;

    fn deref(&self) -> &Self::Target {
        &self.play_ctrl
    }
}

impl DerefMut for Player {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.play_ctrl
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.play_ctrl.set_abort_req(true);
    }
}

fn to_sample(sample_format: cpal::SampleFormat) -> ffmpeg::format::Sample {
    use ffmpeg::format::sample::Type as SampleType;
    use ffmpeg::format::Sample as Sample;
    use cpal::SampleFormat as SampleFormat;

    match &sample_format {
        SampleFormat::I8 => Sample::U8(SampleType::Packed),
        SampleFormat::U8 => Sample::U8(SampleType::Packed),
        SampleFormat::I16 => Sample::I16(SampleType::Packed),
        SampleFormat::U16 => Sample::I16(SampleType::Packed),
        SampleFormat::I32 => Sample::I32(SampleType::Packed),
        SampleFormat::U32 => Sample::I32(SampleType::Packed),
        SampleFormat::I64 => Sample::I64(SampleType::Packed),
        SampleFormat::U64 => Sample::I64(SampleType::Packed),
        SampleFormat::F32 => Sample::F32(SampleType::Packed),
        SampleFormat::F64 => Sample::F64(SampleType::Packed),
        _ => { panic!("SampleFormat do not match") }
    }
}

