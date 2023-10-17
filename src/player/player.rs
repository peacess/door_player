use std::collections::VecDeque;
use std::path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use ffmpeg::media;

type Deque<T> = Arc<parking_lot::Mutex<VecDeque<T>>>;

fn new_deque<T>() -> Deque<T> {
    Arc::new(parking_lot::Mutex::new(VecDeque::new()))
}

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
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

impl Player {
    //初始化所有线程，如果之前的还在，结束它们
    pub fn new(file: &str) -> Result<Player, anyhow::Error> {

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

        let player = Self {
            file_path: file.to_string(),
            stopped: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };

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
                               video_packet_sender, video_index);
        Ok(player)
    }

    fn audio_decode_run(&self, mut audio_decoder: ffmpeg::decoder::Audio, packet_receiver: kanal::Receiver<ffmpeg::Packet>, audio_deque: Deque<ffmpeg::Frame>) {
        let stopped = self.stopped.clone();
        std::thread::spawn(move || {
            loop {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                match packet_receiver.recv() {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(packet) => {
                        match audio_decoder.0.send_packet(&packet) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                    }
                }
                let mut frame = unsafe { ffmpeg::Frame::empty() };
                match audio_decoder.receive_frame(&mut frame) {
                    Err(e) => {
                        log::error!("{}", e);
                    }
                    Ok(_) => {
                        audio_deque.lock().push_back(frame);
                    }
                }
            }
        });
    }

    fn audio_play_run(&self, frame_deque: Deque<ffmpeg::Frame>) {
        let stopped = self.stopped.clone();
        std::thread::spawn(move || {
            loop {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    fn video_decode_run(&self, mut video_decoder: ffmpeg::decoder::Video, packet_receiver: kanal::Receiver<ffmpeg::Packet>, video_deque: Deque<ffmpeg::Frame>) {
        let stopped = self.stopped.clone();
        std::thread::spawn(move || {
            loop {
                if stopped.load(Ordering::Relaxed) {
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
        let stopped = self.stopped.clone();
        std::thread::spawn(move || {
            loop {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                let frame = frame_deque.lock().pop_front();
                if let Some(frame) = frame {}
            }
        });
    }

    fn read_packet_run(&self, mut input: ffmpeg::format::context::Input, audio_deque: kanal::Sender<ffmpeg::Packet>, audio_index: usize,
                       video_deque: kanal::Sender<ffmpeg::Packet>, video_index: usize) {
        let stopped = self.stopped.clone();
        std::thread::spawn(move || {
            loop {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                for (s, packet) in input.packets() {
                    unsafe {
                        if !packet.is_empty() {
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
                }
            }
        });
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if self.stopped.load(std::sync::atomic::Ordering::Relaxed) {
            self.stopped.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

