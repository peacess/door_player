use std::path;
use std::sync::Arc;
use std::time::Duration;

use ffmpeg::media;
use kanal::Receiver;
use parking_lot::RwLock;

use crate::kits::{Deque, new_deque};
use crate::kits::consts::PLAY_MIN_INTERVAL;
use crate::player::audio::{AudioDevice, AudioFrame};
use crate::player::play_ctrl::{Command, PlayCtrl, PlayState};

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
    play_ctrl: PlayCtrl,
    cmd_sender: kanal::Sender<Command>,
}

impl Player {
    //初始化所有线程，如果之前的还在，结束它们
    pub fn new(file: &str) -> Result<Player, anyhow::Error> {
        let (cmd_sender, cmd_receiver) = kanal::bounded::<Command>(2);
        let (state_sender, state_receiver) = kanal::bounded::<PlayState>(1);
        let abort_req = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let play_ctrl = {
            let audio_dev = AudioDevice::new()
                .map_err(|e| {
                    state_sender.send(PlayState::Error(e)).ok();
                })
                .unwrap();
            let audio_dev = Arc::new(RwLock::new(audio_dev));

            PlayCtrl::new(audio_dev, state_sender, abort_req)
        };
        let player = Self {
            file_path: file.to_string(),
            play_ctrl,
            cmd_sender,
        };
        //打开文件
        let format_input = ffmpeg::format::input(&path::Path::new(file))?;

        // 获取视频解码器
        let (video_index, video_decoder) = {
            let video_stream = format_input.streams().best(media::Type::Video).ok_or(ffmpeg::Error::InvalidData)?;
            let video_index = video_stream.index();
            let video_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
            let video_decoder = video_context.decoder().video()?;
            (video_index, video_decoder)
        };

        // 获取音频解码器
        let (audio_index, audio_decoder) = {
            let audio_stream = format_input.streams().best(media::Type::Audio).ok_or(ffmpeg::Error::InvalidData)?;
            let audio_index = audio_stream.index();
            let audio_context = ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())?;
            let audio_decoder = audio_context.decoder().audio()?;
            (audio_index, audio_decoder)
        };

        let (audio_packet_sender, audio_packet_receiver) = kanal::bounded(20);
        let (video_packet_sender, video_packet_receiver) = kanal::bounded(20);

        let audio_frame_deque = new_deque();
        let video_frame_deque = new_deque();

        //开启 音频解码线程
        player.audio_decode_run(audio_decoder, audio_packet_receiver, audio_frame_deque.clone());
        //开启 音频播放线程
        player.audio_play_run(audio_frame_deque);
        //开启 视频解码线程
        player.video_decode_run(video_decoder, video_packet_receiver, video_frame_deque.clone());
        //开启 视频播放
        player.video_play_run(video_frame_deque);
        //开启 读frame线程
        player.read_packet_run(format_input, audio_packet_sender, audio_index,
                               video_packet_sender, video_index, cmd_receiver);
        Ok(player)
    }

    fn audio_decode_run(&self, mut audio_decoder: ffmpeg::decoder::Audio, packet_receiver: kanal::Receiver<ffmpeg::Packet>, audio_deque: Deque<AudioFrame>) {
        let play_ctrl = self.play_ctrl.clone();
        let mut audio_re_sampler = {
            let conf = play_ctrl.audio_default_config();
            match ffmpeg::software::resampling::context::Context::get(
                audio_decoder.format(),
                audio_decoder.channel_layout(),
                audio_decoder.rate(),
                to_sample(conf.sample_format()),
                ffmpeg::ChannelLayout::STEREO,
                conf.sample_rate().0,
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
                    let mut frame = unsafe { ffmpeg::frame::Audio::empty() };
                    match audio_decoder.receive_frame(&mut frame) {
                        Ok(_) => {
                            let mut resampled_frame = ffmpeg::frame::Audio::empty();
                            match audio_re_sampler.run(&frame, &mut resampled_frame) {
                                Err(e) => {
                                    log::error!("{}", e);
                                    continue;
                                }
                                Ok(_) => {
                                    //todo delay
                                }
                            }
                            let samples = if resampled_frame.is_packed() {
                                unsafe {
                                    std::slice::from_raw_parts(
                                        (*frame.as_ptr()).data[0] as *const f32,
                                        frame.samples() * frame.channels() as usize,
                                    )
                                }
                            } else {
                                resampled_frame.plane(0)
                            };
                            let pts = frame.pts().expect("") as f64 / frame.rate() as f64;
                            let duration = frame.samples() as f64 / frame.rate() as f64;
                            let v = play_ctrl.volume();
                            let samples: Vec<f32> = samples.iter().map(|s| s * v).collect();
                            let audio_frame = AudioFrame {
                                samples: samples.into_iter(),
                                channels: frame.channels(),
                                sample_rate: frame.rate(),
                                pts,
                                duration,
                            };
                            audio_deque.lock().push_back(audio_frame);
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

    fn audio_play_run(&self, frame_deque: Deque<AudioFrame>) {
        let play_ctrl = self.play_ctrl.clone();
        std::thread::spawn(move || {
            let mut empty_count = 0;
            loop {
                if play_ctrl.abort_req() {
                    break;
                }
                if play_ctrl.pause() {
                    play_ctrl.wait_notify_in_pause();
                }

                match frame_deque.lock().pop_front() {
                    Some(frame) => {
                        match play_ctrl.play_audio(frame) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                        empty_count = 0;
                        continue;
                    }
                    None => {}
                }

                empty_count += 1;
                if empty_count == 10 {
                    play_ctrl.set_audio_finished(true);
                    break;
                }
                spin_sleep::sleep(PLAY_MIN_INTERVAL);
            }
        });
    }

    fn video_decode_run(&self, mut video_decoder: ffmpeg::decoder::Video, packet_receiver: kanal::Receiver<ffmpeg::Packet>, video_deque: Deque<ffmpeg::Frame>) {
        let play_ctrl = self.play_ctrl.clone();
        std::thread::spawn(move || {
            loop {
                if play_ctrl.abort_req() {
                    break;
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
                let mut frame = unsafe { ffmpeg::Frame::empty() };
                match video_decoder.receive_frame(&mut frame) {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(_) => {
                        // let video_frame = VideoFrame{};
                        video_deque.lock().push_back(frame);
                    }
                }
            }
        });
    }

    fn video_play_run(&self, mut frame_deque: Deque<ffmpeg::Frame>) {
        let play_ctrl = self.play_ctrl.clone();
        std::thread::spawn(move || {
            loop {
                if play_ctrl.abort_req() {
                    break;
                }
                let frame = frame_deque.lock().pop_front();
                if let Some(frame) = frame {}
            }
        });
    }

    fn read_packet_run(&self, mut input: ffmpeg::format::context::Input, audio_deque: kanal::Sender<ffmpeg::Packet>, audio_index: usize,
                       video_deque: kanal::Sender<ffmpeg::Packet>, video_index: usize,
                       cmd_receiver: Receiver<Command>) {
        let mut play_ctrl = self.play_ctrl.clone();
        std::thread::spawn(move || {
            loop {
                if play_ctrl.abort_req() {
                    break;
                }
                match cmd_receiver.try_recv() {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(cmd) => {
                        match cmd {
                            None => {}
                            Some(cmd) => {
                                match cmd {
                                    Command::Terminate => {
                                        play_ctrl.set_abort_req(true);
                                        break;
                                    }
                                    Command::Pause(pause) => {
                                        play_ctrl.set_pause(pause);
                                    }
                                    Command::Mute(mute) => {
                                        play_ctrl.set_mute(mute);
                                    }
                                    Command::Volume(volume) => {
                                        play_ctrl.set_volume(volume);
                                    }
                                }
                            }
                        }
                    }
                }
                if play_ctrl.pause() || audio_deque.is_full() || video_deque.is_full() {
                    spin_sleep::sleep(Duration::from_millis(20));
                    continue;
                }

                if play_ctrl.demux_finished() && play_ctrl.audio_finished() && play_ctrl.video_finished() {
                    play_ctrl.set_abort_req(true);
                    break;
                }

                for (s, packet) in input.packets() {
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
                }

                play_ctrl.set_abort_req(true);
                spin_sleep::sleep(Duration::from_millis(20));
            }
        });
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

    match sample_format {
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

