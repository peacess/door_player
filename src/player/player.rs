use std::default::Default;
use std::ops::{Deref, DerefMut};
use std::path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};
use egui::{Align2, Color32, FontId, Image, Rect, Response, Rounding, Sense, Spinner, Ui, vec2, Vec2};
use egui::epaint::Shadow;
use egui::load::SizedTexture;
use ffmpeg::software::resampling::Context as ResamplingContext;
use kanal::{Receiver, Sender};

use crate::kits::Shared;
use crate::player;
use crate::player::{CommandGo, CommandUi, kits, MAX_DIFF_MOVE_MOUSE, PlayerState, SubtitlePlayFrame};
use crate::player::audio::{AudioDevice, AudioPlayFrame};
use crate::player::consts::{AUDIO_FRAME_QUEUE_SIZE, AUDIO_PACKET_QUEUE_SIZE, PLAY_MIN_INTERVAL, VIDEO_FRAME_QUEUE_SIZE, VIDEO_PACKET_QUEUE_SIZE};
use crate::player::play_ctrl::PlayCtrl;
use crate::player::video::VideoPlayFrame;

/// player base ffmpeg, there are 4 threads to player file.
pub struct Player {
    //是否需要停止播放相关线程
    pub play_ctrl: PlayCtrl,
    pub width: u32,
    pub height: u32,

    last_seek_ms: Option<i64>,
    //按一次tab 前进的时间，默认为0
    pub tab_seek_ms: i64,

    /// mouse move ts, compute if show the status bar
    pub mouth_move_ts: i64,

    pub command_ui: Shared<CommandUi>,
}

impl Player {
    //初始化所有线程，如果之前的还在，结束它们
    pub fn new(ctx: &egui::Context, mut texture_handle: egui::TextureHandle, command_ui: Shared<CommandUi>, file: &String) -> Result<Player, anyhow::Error> {
        {
            let mut format_input = ffmpeg::format::input(&path::Path::new(file))?;
            match Self::first_frame(&mut format_input) {
                Ok(f) => texture_handle.set(Self::frame_to_color_image(&f)?, egui::TextureOptions::LINEAR),
                Err(e) => log::error!("{}", e),
            }
            let _ = print_meda_info(&mut format_input);
        }

        let video_input = ffmpeg::format::input(&path::Path::new(file))?;
        // 获取视频解码器
        let (video_index, video_decoder, video_stream_time_base) = {
            let video_stream = video_input.streams().best(ffmpeg::media::Type::Video);
            match video_stream {
                Some(video_stream) => {
                    let video_index = video_stream.index();

                    #[cfg(not(feature = "meh_ffmpeg"))]
                        let mut video_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
                    #[cfg(feature = "meh_ffmpeg")]
                        let mut video_context = ffmpeg::codec::context::Context::new();
                    {
                        #[cfg(feature = "meh_ffmpeg")]
                        video_context.set_parameters(video_stream.parameters());
                        let mut thread_conf = video_context.threading();
                        log::info!("video threads default : {:?}", &thread_conf);
                        thread_conf.count = 2;
                        thread_conf.kind = ffmpeg::threading::Type::Slice;
                        log::info!("video threads new : {:?}", &thread_conf);
                        video_context.set_threading(thread_conf);
                    }
                    // ffmpeg::codec::Context::new()

                    let video_decoder = video_context.decoder().video()?;
                    #[cfg(not(feature = "meh_ffmpeg"))]
                    {
                        log::info!("video_stream time base: {}", video_stream.time_base());
                        log::info!("video_decoder time base: {}", video_decoder.time_base());
                        (video_index, Some(video_decoder), Some(video_stream.time_base()))
                    }
                    #[cfg(feature = "meh_ffmpeg")]
                    {
                        log::info!("video_stream time base: {}", video_stream.time_base().expect(""));
                        log::info!("video_decoder time base: {}", video_decoder.time_base().expect(""));
                        (video_index, Some(video_decoder), video_stream.time_base())
                    }
                }
                None => (0, None, None)
            }
        };

        // let video_input = ffmpeg::format::input(&path::Path::new(file))?;
        // 获取音频解码器
        let (audio_index, audio_decoder, audio_stream_time_base) = {
            let audio_stream = video_input.streams().best(ffmpeg::media::Type::Audio);
            if let Some(audio_stream) = audio_stream {
                let audio_index = audio_stream.index();
                #[cfg(not(feature = "meh_ffmpeg"))]
                    let mut audio_context = ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())?;
                #[cfg(feature = "meh_ffmpeg")]
                    let mut audio_context = ffmpeg::codec::context::Context::new();
                {
                    #[cfg(feature = "meh_ffmpeg")]
                    audio_context.set_parameters(audio_stream.parameters());

                    let mut thread_conf = audio_context.threading();
                    log::info!("audio threads default : {:?}", &thread_conf);
                    thread_conf.count = 2;
                    thread_conf.kind = ffmpeg::threading::Type::Slice;
                    log::info!("audio threads new : {:?}", &thread_conf);
                    audio_context.set_threading(thread_conf);
                }
                let audio_decoder = audio_context.decoder().audio()?;
                #[cfg(not(feature = "meh_ffmpeg"))]
                {
                    log::info!("audio_stream time base: {}", audio_stream.time_base());
                    log::info!("audio_decoder time base: {}", audio_decoder.time_base());
                    (audio_index, Some(audio_decoder), Some(audio_stream.time_base()))
                }
                #[cfg(feature = "meh_ffmpeg")]
                {
                    log::info!("audio_stream time base: {}", audio_stream.time_base().expect(""));
                    log::info!("audio_decoder time base: {}", audio_decoder.time_base().expect(""));
                    (audio_index, Some(audio_decoder), audio_stream.time_base())
                }
            } else {
                (0, None, None)
            }
        };

        // 字幕
        let graph = {
            match &video_decoder {
                None => None,
                Some(video_decoder) => {
                    #[cfg(not(feature = "meh_ffmpeg"))]
                        let video_time_base = video_input.stream(video_index).expect("").time_base();
                    #[cfg(feature = "meh_ffmpeg")]
                        let video_time_base = video_input.stream(video_index).expect("").time_base().expect("");

                    let sub_title_file = {
                        if video_input.streams().best(ffmpeg::media::Type::Subtitle).is_some() {
                            file.clone()
                        } else {
                            if let Some(f) = kits::SubTitle::sub_files(file).first() {
                                f.to_str().expect("").to_string()
                            } else {
                                "".to_string()
                            }
                        }
                    };

                    if !sub_title_file.is_empty() {
                        match Self::graph(&video_decoder, &sub_title_file, video_time_base) {
                            Err(e) => {
                                log::error!("{}", e);
                                None
                            }
                            Ok(t) => Some(t)
                        }
                    } else {
                        None
                    }
                }
            }
        };

        let mut player = {
            #[cfg(not(feature = "meh_ffmpeg"))]
                let duration = video_input.duration();
            #[cfg(feature = "meh_ffmpeg")]
                let duration = video_input.duration().expect("");

            let play_ctrl = {
                let (producer, consumer) = ringbuf::HeapRb::<f32>::new(8820).split();
                let audio_dev = Arc::new(AudioDevice::new(consumer)?);
                audio_dev.resume();
                PlayCtrl::new(duration, producer, audio_dev, texture_handle, video_stream_time_base, audio_stream_time_base)
            };
            Self {
                play_ctrl,
                width: 0,
                height: 0,
                last_seek_ms: None,
                tab_seek_ms: 0,
                mouth_move_ts: Utc::now().timestamp_millis(),
                command_ui,
            }
        };
        if let Some(video_decoder) = &video_decoder {
            player.width = video_decoder.width();
            player.height = video_decoder.height();
        }

        let video_packet_sender = match video_decoder {
            None => None,
            Some(video_decoder) => {
                let (video_packet_sender, video_packet_receiver) = {
                    if VIDEO_PACKET_QUEUE_SIZE < 1 {
                        kanal::unbounded()
                    } else {
                        kanal::bounded(VIDEO_PACKET_QUEUE_SIZE)
                    }
                };
                let (video_play_sender, video_play_receiver) = kanal::bounded(VIDEO_FRAME_QUEUE_SIZE);
                {
                    player.video_packet_receiver = Some(video_packet_receiver.clone());
                    player.video_packet_sender = Some(video_packet_sender.clone());
                    player.video_play_receiver = Some(video_play_receiver.clone());
                    player.video_play_sender = Some(video_play_sender.clone());
                    player.video_stream_time_base = video_stream_time_base;
                }
                //run decode video thread
                player.video_decode_run(video_decoder, video_packet_receiver, video_play_sender, graph);
                //run play video thread
                player.video_play_run(ctx.clone(), video_play_receiver);

                Some(video_packet_sender)
            }
        };

        let audio_packet_sender = match audio_decoder {
            Some(audio_decoder) => {
                let (audio_packet_sender, audio_packet_receiver) = kanal::bounded(AUDIO_FRAME_QUEUE_SIZE);
                let (audio_play_sender, audio_play_receiver) = kanal::bounded(AUDIO_PACKET_QUEUE_SIZE);
                {
                    player.audio_packet_receiver = Some(audio_packet_receiver.clone());
                    player.audio_packet_sender = Some(audio_packet_sender.clone());
                    player.audio_play_receiver = Some(audio_play_receiver.clone());
                    player.audio_play_sender = Some(audio_play_sender.clone());
                    player.audio_stream_time_base = audio_stream_time_base;
                }
                //run audio decode thread
                player.audio_decode_run(audio_decoder, audio_packet_receiver, audio_play_sender);
                //run audio play thread
                player.audio_play_run(audio_play_receiver);
                Some(audio_packet_sender)
            }
            None => None
        };

        if audio_packet_sender.is_some() || video_packet_sender.is_some() {
            player.read_packet_run(video_input, audio_packet_sender, audio_index,
                                   video_packet_sender, video_index);
            Ok(player)
        } else {
            Err(anyhow::Error::new(ffmpeg::Error::StreamNotFound))
        }
        // player.play_ctrl.set_pause(false);
    }

    pub fn default_texture_handle(ctx: &egui::Context) -> egui::TextureHandle {
        let img = egui::ColorImage::new([124, 124], Color32::TRANSPARENT);
        let texture_handle = ctx.load_texture("video_stream_default", img, egui::TextureOptions::LINEAR);
        texture_handle
    }

    pub fn frame_to_color_image(frame: &ffmpeg::frame::Video) -> Result<egui::ColorImage, ffmpeg::Error> {
        let mut rgb_frame = ffmpeg::frame::Video::empty();
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
        let mut pixels = Vec::with_capacity(height * rgb_frame.width() as usize / pixel_size_bytes);
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

    //, time_base_video: ffmpeg::Rational
    fn graph(dec_ctx: &ffmpeg::decoder::Video, sub_title_file: &str, time_base: ffmpeg::Rational) -> Result<ffmpeg::filter::Graph, ffmpeg::Error> {
        let mut graph = ffmpeg::filter::Graph::new();
        let src = ffmpeg::filter::find("buffer").ok_or(ffmpeg::Error::OptionNotFound)?;
        let sink = ffmpeg::filter::find("buffersink").ok_or(ffmpeg::Error::OptionNotFound)?;
        #[cfg(not(feature = "meh_ffmpeg"))]
            let args = format!("video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
                               dec_ctx.width(), dec_ctx.height(), ffmpeg::ffi::AVPixelFormat::from(dec_ctx.format()) as i32,
                               time_base.numerator(), time_base.denominator(),
                               dec_ctx.aspect_ratio().numerator(),
                               dec_ctx.aspect_ratio().denominator());
        #[cfg(feature = "meh_ffmpeg")]
            let args = format!("video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
                               dec_ctx.width(), dec_ctx.height(), ffmpeg::ffi::AVPixelFormat::from(dec_ctx.format().into()) as i32,
                               time_base.numerator(), time_base.denominator(),
                               dec_ctx.aspect_ratio().numerator(),
                               dec_ctx.aspect_ratio().denominator());
        log::info!("{}",args);
        let _ = graph.add(&src, "in", &args)?;
        let _ = graph.add(&sink, "out", "")?;
        let mut parse = graph.input("out", 0)?;
        parse = parse.output("in", 0)?;
        // let file = ""
        // let spec = format!("subtitles=filename='{}':original_size={}x{}","/home/peace/gopath/src/peacess/door_player/13.mkv",dec_ctx.width(),dec_ctx.height());
        let spec = format!("subtitles=filename='{}'", sub_title_file);
        parse.parse(&spec)?;
        graph.validate()?;
        Ok(graph)
    }

    #[cfg(not(feature = "meh_ffmpeg"))]
    fn first_frame(input: &mut ffmpeg::format::context::Input) -> Result<ffmpeg::frame::Video, anyhow::Error> {
        let video_stream = input.streams().best(ffmpeg::media::Type::Video).ok_or(ffmpeg::Error::InvalidData)?;
        let video_index = video_stream.index();
        let video_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
        let mut video_decoder = video_context.decoder().video()?;
        loop {
            if let Some((_, packet)) = input.packets().next() {
                if unsafe { packet.is_empty() || packet.stream() != video_index } {
                    continue;
                }
                video_decoder.send_packet(&packet)?;
                let mut frame = ffmpeg::frame::Video::empty();
                match video_decoder.receive_frame(&mut frame) {
                    Err(e) => {
                        log::debug!("{}", e);
                    }
                    Ok(_) => {
                        return Ok(frame);
                    }
                }
            }
        }
    }

    #[cfg(feature = "meh_ffmpeg")]
    fn first_frame(input: &mut ffmpeg::format::context::Input) -> Result<ffmpeg::frame::Video, anyhow::Error> {
        let video_stream = input.streams().best(ffmpeg::media::Type::Video).ok_or(ffmpeg::Error::InvalidData)?;
        let video_index = video_stream.index();
        let mut video_context = ffmpeg::codec::context::Context::new();
        video_context.set_parameters(video_stream.parameters());
        let mut video_decoder = video_context.decoder().video()?;
        loop {
            if let Some(Ok((_, packet))) = input.packets().next() {
                if unsafe { packet.is_empty() || packet.stream() != video_index } {
                    continue;
                }
                video_decoder.send_packet(&packet)?;
                let mut frame = ffmpeg::frame::Video::empty();
                match video_decoder.receive_frame(&mut frame) {
                    Err(e) => {
                        log::debug!("{}", e);
                    }
                    Ok(_) => {
                        return Ok(frame);
                    }
                }
            }
        }
    }

    fn audio_decode_run(&self, mut audio_decoder: ffmpeg::decoder::Audio, audio_packet_receiver: Receiver<Option<ffmpeg::Packet>>, audio_play_sender: Sender<AudioPlayFrame>) {
        let play_ctrl = self.play_ctrl.clone();
        let mut audio_re_sampler = {
            let stream_config = play_ctrl.audio_config();
            #[cfg(not(feature = "meh_ffmpeg"))]
            match ResamplingContext::get(
                audio_decoder.format(),
                audio_decoder.channel_layout(),
                audio_decoder.rate() as u32,
                to_sample(stream_config.sample_format()),
                ffmpeg::ChannelLayout::default(stream_config.channels() as i32),//ffmpeg::ChannelLayout::STEREO,
                stream_config.sample_rate().0,
            ) {
                Err(e) => {
                    log::error!("{}", e);
                    panic!("{}", e);
                }
                Ok(t) => t
            }
            #[cfg(feature = "meh_ffmpeg")]
            match ResamplingContext::get(
                audio_decoder.format(),
                audio_decoder.channel_layout(),
                audio_decoder.sample_rate(),
                to_sample(stream_config.sample_format()),
                ffmpeg::ChannelLayout::default(stream_config.channels() as i32),//ffmpeg::ChannelLayout::STEREO,
                stream_config.sample_rate().0,
            ) {
                Err(e) => {
                    log::error!("{}", e);
                    panic!("{}", e);
                }
                Ok(t) => t
            }
        };
        let _ = std::thread::Builder::new().name("audio decode".to_string()).spawn(move || {
            'RUN: loop {
                match play_ctrl.player_state.get() {
                    PlayerState::Stopped => {
                        log::info!("audio decode exit");
                        break 'RUN;
                    }
                    PlayerState::EndOfFile => {}
                    PlayerState::Seeking(_) => {}
                    PlayerState::Paused => {
                        spin_sleep::sleep(PLAY_MIN_INTERVAL);
                        continue 'RUN;
                    }
                    PlayerState::Playing => {}
                    PlayerState::Restarting => {}
                }

                loop {
                    match play_ctrl.player_state.get() {
                        PlayerState::Stopped => {
                            log::info!("audio decode exit");
                            break 'RUN;
                        }
                        PlayerState::EndOfFile => {}
                        PlayerState::Seeking(_) => {}
                        PlayerState::Paused => {
                            spin_sleep::sleep(PLAY_MIN_INTERVAL);
                            continue 'RUN;
                        }
                        PlayerState::Playing => {}
                        PlayerState::Restarting => {}
                    }
                    let mut frame_old = ffmpeg::frame::Audio::empty();
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
                            let v = play_ctrl.audio_volume.get() as f32;
                            let samples: Vec<f32> = re_samples_ref.iter().map(|s| s * v).collect();
                            let (duration, pts) = {
                                let packet_frame = frame_old.packet();
                                let pts = match frame_old.pts() {
                                    None => {
                                        log::info!("Frame pts is none");
                                        0
                                    }
                                    Some(t) => t
                                };
                                (packet_frame.duration, pts)
                            };
                            #[cfg(not(feature = "meh_ffmpeg"))]
                                let audio_frame = AudioPlayFrame {
                                samples,
                                channels: frame_resample.channels(),
                                sample_rate: frame_resample.rate(),
                                pts,
                                duration,
                            };
                            #[cfg(feature = "meh_ffmpeg")]
                                let audio_frame = AudioPlayFrame {
                                samples,
                                channels: frame_resample.channels(),
                                sample_rate: frame_resample.sample_rate(),
                                pts,
                                duration,
                            };
                            match audio_play_sender.send(audio_frame) {
                                Err(e) => {
                                    log::error!("{}", e);
                                }
                                Ok(_) => {}
                            }
                            spin_sleep::sleep(PLAY_MIN_INTERVAL);
                        }
                        Err(e) => {
                            log::debug!("{}", e);
                            break;
                        }
                    }
                }

                match audio_packet_receiver.try_recv() {
                    Err(e) => {
                        log::error!("{}", e);
                        break 'RUN;
                    }
                    Ok(Some(Some(packet))) => {
                        if PlayerState::Stopped == play_ctrl.player_state.get() {
                            log::info!("audio decode exit");
                            break 'RUN;
                        }
                        match audio_decoder.send_packet(&packet) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                    }
                    Ok(None) => {
                        spin_sleep::sleep(std::time::Duration::from_millis(2));
                        continue 'RUN;
                    }
                    Ok(Some(None)) => {
                        //receive all frame
                        let mut temp = ffmpeg::frame::Audio::empty();
                        for _ in 0..20 {
                            if let Err(_) = audio_decoder.receive_frame(&mut temp) {
                                break;
                            }
                        }
                        spin_sleep::sleep(std::time::Duration::from_millis(2));
                        continue 'RUN;
                    }
                }
            }
        });
    }

    fn audio_play_run(&self, audio_play_receiver: Receiver<AudioPlayFrame>) {
        let mut play_ctrl = self.play_ctrl.clone();
        let _ = std::thread::Builder::new().name("audio play".to_string()).spawn(move || {
            let mut empty_count = 0;
            loop {
                if play_ctrl.player_state.get() == PlayerState::Stopped {
                    log::info!("audio play exit");
                    break;
                }
                match audio_play_receiver.try_recv() {
                    Err(e) => {
                        log::error!("{}", e);
                        empty_count += 1;
                        if empty_count == 10 {
                            play_ctrl.set_audio_finished(true);
                            log::info!("audio play exit");
                            break;
                        }
                    }
                    Ok(None) => {
                        if play_ctrl.packet_finished() {
                            empty_count += 1;
                            if empty_count == 10 {
                                play_ctrl.set_audio_finished(true);
                                log::info!("audio play exit");
                                break;
                            }
                        }
                    }
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
                spin_sleep::sleep(PLAY_MIN_INTERVAL);
            }
        });
    }

    fn video_decode_run(&self, mut video_decoder: ffmpeg::decoder::Video, video_packet_receiver: kanal::Receiver<Option<ffmpeg::Packet>>, video_play_sender: Sender<VideoPlayFrame>, mut graph: Option<ffmpeg::filter::Graph>) {
        let play_ctrl = self.play_ctrl.clone();
        let width = video_decoder.width() as usize;
        let height = video_decoder.height() as usize;

        let _ = std::thread::Builder::new().name("video decode".to_string()).spawn(move || 'RUN: loop {
            match play_ctrl.player_state.get() {
                PlayerState::Stopped => {
                    log::info!("video decode exit");
                    break 'RUN;
                }
                PlayerState::EndOfFile => {}
                PlayerState::Seeking(_) => {}
                PlayerState::Paused => {
                    spin_sleep::sleep(PLAY_MIN_INTERVAL);
                    continue 'RUN;
                }
                PlayerState::Playing => {}
                PlayerState::Restarting => {}
            }

            match video_packet_receiver.try_recv() {
                Err(e) => {
                    log::error!("{}", e);
                    // spin_sleep::sleep(PLAY_MIN_INTERVAL);
                }
                Ok(Some(Some(packet))) => {
                    match video_decoder.send_packet(&packet) {
                        Err(e) => {
                            log::error!("{}", e);
                        }
                        Ok(_) => {}
                    }
                    // spin_sleep::sleep(std::time::Duration::from_millis(2));
                }
                Ok(None) => {
                    spin_sleep::sleep(std::time::Duration::from_millis(2));
                    continue;
                }
                Ok(Some(None)) => {
                    //receive all frame
                    let mut temp = ffmpeg::frame::Video::empty();
                    for _ in 0..20 {
                        if let Err(_) = video_decoder.receive_frame(&mut temp) {
                            break;
                        }
                    }
                    spin_sleep::sleep(std::time::Duration::from_millis(2));
                    continue;
                }
            }
            loop {
                match play_ctrl.player_state.get() {
                    PlayerState::Stopped => {
                        log::info!("video decode exit");
                        break 'RUN;
                    }
                    PlayerState::EndOfFile => {}
                    PlayerState::Seeking(_) => {}
                    PlayerState::Paused => {
                        spin_sleep::sleep(PLAY_MIN_INTERVAL);
                        continue 'RUN;
                    }
                    PlayerState::Playing => {}
                    PlayerState::Restarting => {}
                }
                let mut v_frame = ffmpeg::frame::Video::empty();
                if let Err(e) = video_decoder.receive_frame(&mut v_frame) {
                    log::debug!("{}", e);
                    continue 'RUN;
                } else {
                    let mut err_count = 0;
                    let frame = {
                        match graph {
                            None => v_frame,
                            Some(ref mut graph) => {
                                loop {
                                    let mut filter_frame = ffmpeg::frame::Video::empty();
                                    if let Err(e) = graph.get("in").expect("").source().add(&v_frame) {
                                        log::error!("{}", e);
                                        continue 'RUN;
                                    }

                                    if let Err(e) = graph.get("out").expect("").sink().frame(&mut filter_frame) {
                                        log::error!("{}", e);
                                        err_count += 1;
                                    } else {
                                        break filter_frame;
                                    }
                                    if err_count > 3 {
                                        continue 'RUN;
                                    }
                                }
                            }
                        }
                    };
                    let color_image = match Self::frame_to_color_image(&frame) {
                        Err(e) => {
                            log::error!("{}", e);
                            continue;
                        }
                        Ok(t) => t,
                    };

                    let (duration, pts) = {
                        let packet_frame = frame.packet();
                        let pts = match frame.pts() {
                            None => {
                                log::debug!("Frame pts is none");
                                match frame.timestamp() {
                                    Some(t) => t,
                                    None => {
                                        unsafe {
                                            match (*frame.as_ptr()).pkt_dts {
                                                ffmpeg::ffi::AV_NOPTS_VALUE => 0,
                                                t => t,
                                            }
                                        }
                                    }
                                }
                            }
                            Some(t) => t
                        };
                        (packet_frame.duration, pts)
                    };

                    let video_frame = VideoPlayFrame {
                        width,
                        height,
                        pts,
                        duration,
                        color_image,
                    };
                    match video_play_sender.send(video_frame) {
                        Err(e) => {
                            log::error!("{}", e);
                        }
                        Ok(_) => {}
                    }
                    // spin_sleep::sleep(std::time::Duration::from_millis(2));
                }
            }
        });
    }

    fn video_play_run(&self, ctx: egui::Context, video_play_receiver: Receiver<VideoPlayFrame>) {
        let mut play_ctrl = self.play_ctrl.clone();
        let _ = std::thread::Builder::new().name("video play".to_string()).spawn(move || {
            let mut empty_count = 0;
            loop {
                if PlayerState::Stopped == play_ctrl.player_state.get() {
                    log::info!("video play exit");
                    break;
                }
                match play_ctrl.player_state.get() {
                    PlayerState::Stopped => {
                        log::info!("video play exit");
                        break;
                    }
                    PlayerState::EndOfFile => {}
                    PlayerState::Seeking(_) => {}
                    PlayerState::Paused => {
                        spin_sleep::sleep(PLAY_MIN_INTERVAL);
                        continue;
                    }
                    PlayerState::Playing => {}
                    PlayerState::Restarting => {}
                }

                if let CommandGo::Frame(t) = play_ctrl.command_go.get() {
                    play_ctrl.command_go.set(CommandGo::None);
                    for _ in 1..t {
                        loop {
                            if let Ok(Some(_)) = video_play_receiver.try_recv() {
                                break;
                            }
                        }
                    }
                }

                match video_play_receiver.try_recv() {
                    Err(e) => {
                        log::error!("{}", e);
                        empty_count += 1;
                        if empty_count == 10 {
                            play_ctrl.set_video_finished(true);
                            log::info!("video play exit");
                            break;
                        }
                    }
                    Ok(None) => {
                        if play_ctrl.packet_finished() {
                            empty_count += 1;
                            if empty_count == 10 {
                                play_ctrl.set_video_finished(true);
                                log::info!("video play exit");
                                break;
                            }
                        }
                    }
                    Ok(Some(frame)) => {
                        if let Err(e) = play_ctrl.play_video(frame, &ctx) {
                            log::error!("{}", e);
                        }
                        empty_count = 0;
                        continue;
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        });
    }

    ///  [ass to image](https://www.cnblogs.com/tocy/p/subtitle-format-libass-tutorial.html)
    /// [merge frame] https://github.com/nldzsz/ffmpeg-demo
    /// the fn do not use, replace to subtitle filter
    #[allow(dead_code)]
    fn subtitle_decode_run(&self, mut subtitle_decoder: ffmpeg::decoder::Subtitle, subtitle_packet_receiver: kanal::Receiver<Option<ffmpeg::Packet>>, subtitle_play_sender: Sender<SubtitlePlayFrame>) {
        let play_ctrl = self.play_ctrl.clone();
        let _ = std::thread::Builder::new().name("subtitle decode".to_string()).spawn(move || loop {
            if PlayerState::Stopped == play_ctrl.player_state.get() {
                log::info!("subtitle decode exit");
                break;
            }
            // use ffmpeg and ass to render subtitle

            match subtitle_packet_receiver.recv() {
                Err(e) => {
                    log::error!("{}", e);
                }
                Ok(None) => {}
                Ok(Some(packet)) => {
                    let mut sub = ffmpeg::Subtitle::default();
                    match subtitle_decoder.decode(&packet, &mut sub) {
                        Err(e) => {
                            log::error!("{}", e);
                        }
                        Ok(b) => {
                            if b {
                                #[cfg(not(feature = "meh_ffmpeg"))]
                                    let mut subtitle_frame = SubtitlePlayFrame {
                                    pts: sub.pts().unwrap_or_default() as f64,
                                    duration: packet.duration(),
                                    ..Default::default()
                                };
                                #[cfg(feature = "meh_ffmpeg")]
                                    let mut subtitle_frame = SubtitlePlayFrame {
                                    pts: sub.pts().unwrap_or_default() as f64,
                                    duration: packet.duration().expect(""),
                                    ..Default::default()
                                };
                                let mut sub_text = String::default();
                                for rect in sub.rects() {
                                    let line = match rect {
                                        ffmpeg::subtitle::Rect::None(_) => String::default(),
                                        ffmpeg::subtitle::Rect::Bitmap(bitmap) => {
                                            subtitle_frame.width = bitmap.width();
                                            subtitle_frame.height = bitmap.height();
                                            //todo
                                            String::default()
                                        }
                                        ffmpeg::subtitle::Rect::Text(text) => {
                                            text.get().to_string()
                                        }
                                        ffmpeg::subtitle::Rect::Ass(ass) => {
                                            ass.get().to_string()
                                        }
                                    };
                                    if sub_text.is_empty() {
                                        sub_text = line;
                                    } else {
                                        sub_text.push_str("\n");
                                        sub_text.push_str(&line);
                                    }
                                }
                                subtitle_frame.pure_text = sub_text;

                                match subtitle_play_sender.send(subtitle_frame) {
                                    Err(e) => {
                                        log::error!("{}", e);
                                    }
                                    Ok(_) => {}
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    fn read_packet_run(&self, mut input: ffmpeg::format::context::Input, audio_packet_sender: Option<kanal::Sender<Option<ffmpeg::Packet>>>, audio_index: usize,
                       video_packet_sender: Option<kanal::Sender<Option<ffmpeg::Packet>>>, video_index: usize) {
        let play_ctrl = self.play_ctrl.clone();
        #[cfg(not(feature = "meh_ffmpeg"))]
            let duration = input.duration();
        #[cfg(feature = "meh_ffmpeg")]
            let duration = input.duration().expect("");
        let _ = std::thread::Builder::new().name("read packet".to_string()).spawn(move || {
            'PACKETS: loop {
                if play_ctrl.player_state.get() == PlayerState::Stopped {
                    log::info!("read packet exit");
                    break;
                }

                if (audio_packet_sender.is_none() || play_ctrl.audio_finished()) && (video_packet_sender.is_none() || play_ctrl.video_finished()) {
                    play_ctrl.player_state.set(PlayerState::Stopped);
                    log::info!("read packet exit");
                    break;
                }

                let mut packets = 1;
                match play_ctrl.command_go.get() {
                    CommandGo::Packet(next_amount) => {
                        play_ctrl.command_go.set(CommandGo::None);
                        for _ in 1..next_amount {
                            if let None = input.packets().next() {
                                play_ctrl.set_packet_finished(true);
                                spin_sleep::sleep(PLAY_MIN_INTERVAL);
                                continue 'PACKETS;
                            }
                        }
                    }
                    CommandGo::GoMs(ms) => {
                        log::info!("go ms: {}", ms);
                        play_ctrl.command_go.set(CommandGo::None);
                        let mut diff = play_ctrl.elapsed_ms() + ms;
                        if diff < 0 {
                            diff = 0;
                        }
                        let seek_pos = (diff * duration) / play_ctrl.duration_ms;
                        {
                            let re = if ms > 0 {
                                input.seek(seek_pos, seek_pos..)
                            } else {
                                input.seek(seek_pos, ..seek_pos)
                            };
                            if let Err(e) = re {
                                log::error!("{}", e);
                            }
                        }

                        play_ctrl.seek_clean();
                        if let Some(a) = &audio_packet_sender {
                            let _ = a.send(None);
                        }
                        if let Some(v) = &video_packet_sender {
                            let _ = v.send(None);
                        }
                        if let PlayerState::Paused = play_ctrl.player_state.get() {
                            packets = 2;
                        }
                    }
                    CommandGo::Seek(t) => {
                        play_ctrl.command_go.set(CommandGo::None);
                        let seek_pos = {
                            if t > play_ctrl.duration {
                                play_ctrl.duration
                            } else if t < 1 {
                                0
                            } else {
                                t
                            }
                        };
                        if let Err(e) = input.seek(seek_pos, ..seek_pos) {
                            log::error!("{}", e);
                        }
                        play_ctrl.seek_clean();
                        if let Some(a) = &audio_packet_sender {
                            let _ = a.send(None);
                        }
                        if let Some(v) = &video_packet_sender {
                            let _ = v.send(None);
                        }

                        //不是每一packet的数据都会有界面输出，所以会出现seek后且是pause时，画面没有到位，所以多输出一packet
                        if let PlayerState::Paused = play_ctrl.player_state.get() {
                            packets = 2;
                        }
                    }
                    _ => {
                        if play_ctrl.player_state.get() == PlayerState::Paused
                            || (audio_packet_sender.is_some() && audio_packet_sender.as_ref().expect("").is_full())
                            || (video_packet_sender.is_some() && video_packet_sender.as_ref().expect("").is_full()) {
                            spin_sleep::sleep(PLAY_MIN_INTERVAL);
                            continue 'PACKETS;
                        }
                    }
                }

                #[cfg(not(feature = "meh_ffmpeg"))]
                for _ in 0..packets {
                    if let Some((_, packet)) = input.packets().next() {
                        if unsafe { !packet.is_empty() } {
                            if audio_packet_sender.is_some() && packet.stream() == audio_index {
                                if let Err(e) = audio_packet_sender.as_ref().expect("").send(Some(packet)) {
                                    log::error!("{}", e);
                                }
                            } else if video_packet_sender.is_some() && packet.stream() == video_index {
                                if let Err(e) = video_packet_sender.as_ref().expect("").send(Some(packet)) {
                                    log::error!("{}", e);
                                }
                            }
                            // } else if packet.stream() == subtitle_index {
                            //     if let Err(e) = subtitle_deque.send(Some(packet)) {
                            //         log::error!("{}", e);
                            //     }
                            // }
                        }
                    } else {
                        play_ctrl.set_packet_finished(true);
                        spin_sleep::sleep(PLAY_MIN_INTERVAL);
                        continue 'PACKETS;
                    }
                }

                #[cfg(feature = "meh_ffmpeg")]
                for _ in 0..packets {
                    match input.packets().next() {
                        Some(Ok((_, packet))) => {
                            if unsafe { !packet.is_empty() } {
                                if audio_packet_sender.is_some() && packet.stream() == audio_index {
                                    if let Err(e) = audio_packet_sender.as_ref().expect("").send(Some(packet)) {
                                        log::error!("{}", e);
                                    }
                                } else if video_packet_sender.is_some() && packet.stream() == video_index {
                                    if let Err(e) = video_packet_sender.as_ref().expect("").send(Some(packet)) {
                                        log::error!("{}", e);
                                    }
                                }
                                // } else if packet.stream() == subtitle_index {
                                //     if let Err(e) = subtitle_deque.send(Some(packet)) {
                                //         log::error!("{}", e);
                                //     }
                                // }
                            }
                        }
                        Some(Err(e)) => {
                            play_ctrl.set_packet_finished(true);
                            spin_sleep::sleep(PLAY_MIN_INTERVAL);
                            continue 'PACKETS;
                        }
                        None => {
                            play_ctrl.set_packet_finished(true);
                            spin_sleep::sleep(PLAY_MIN_INTERVAL);
                            continue 'PACKETS;
                        }
                    }
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
        self.render_ui(ui, &response);
        self.process_state();
        response
    }
    fn render_ui(&mut self, ui: &mut Ui, image_res: &Response) -> Option<Rect> {
        let hovered = ui.rect_contains_pointer(image_res.rect);
        let currently_seeking = if let PlayerState::Seeking(_) = self.player_state.get() { true } else { false };
        let is_stopped = self.player_state.get() == PlayerState::Stopped;
        let is_paused = self.player_state.get() == PlayerState::Paused;
        let seekbar_anim_frac = ui.ctx().animate_bool_with_time(
            image_res.id.with("seekbar_anim"),
            hovered || currently_seeking || is_paused || is_stopped,
            0.2,
        );

        {
            let mut moving = false;
            ui.input(|e| {
                if e.pointer.is_moving() {
                    moving = true;
                }
            });
            if moving {
                self.mouth_move_ts = chrono::Utc::now().timestamp_millis();
                let cursor = ui.ctx().output(|o| o.cursor_icon);
                if cursor == egui::CursorIcon::None {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }
            }
        };

        if (is_paused || is_stopped || currently_seeking || self.show_seekbar()) && seekbar_anim_frac > 0. {
            let seekbar_width_offset = 20.;
            let full_seek_bar_width = image_res.rect.width() - seekbar_width_offset;

            let seekbar_width = full_seek_bar_width * self.duration_frac();

            let seekbar_offset = 20.;
            let seekbar_pos = image_res.rect.left_bottom()
                + vec2(seekbar_width_offset / 2., -seekbar_offset);
            let seekbar_height = 3.;
            let mut full_seek_bar_rect =
                Rect::from_min_size(seekbar_pos, vec2(full_seek_bar_width, seekbar_height));

            let mut seekbar_rect =
                Rect::from_min_size(seekbar_pos, vec2(seekbar_width, seekbar_height));
            let seekbar_interact_rect = full_seek_bar_rect.expand(10.);
            ui.interact(seekbar_interact_rect, image_res.id, Sense::drag());

            let seekbar_response = ui.interact(
                seekbar_interact_rect,
                image_res.id.with("seekbar"),
                Sense::click_and_drag(),
            );

            let seekbar_hovered = seekbar_response.hovered();
            let seekbar_hover_anim_frac = ui.ctx().animate_bool_with_time(
                image_res.id.with("seekbar_hover_anim"),
                seekbar_hovered || currently_seeking,
                0.2,
            );

            if seekbar_hover_anim_frac > 0. {
                let new_top = full_seek_bar_rect.top() - (3. * seekbar_hover_anim_frac);
                full_seek_bar_rect.set_top(new_top);
                seekbar_rect.set_top(new_top);
            }

            let seek_indicator_anim = ui.ctx().animate_bool_with_time(
                image_res.id.with("seek_indicator_anim"),
                currently_seeking,
                0.1,
            );

            if currently_seeking {
                let mut seek_indicator_shadow = Shadow::big_dark();
                seek_indicator_shadow.color = seek_indicator_shadow
                    .color
                    .linear_multiply(seek_indicator_anim);
                let spinner_size = 20. * seek_indicator_anim;
                ui.painter().add(
                    seek_indicator_shadow.tessellate(image_res.rect, Rounding::ZERO),
                );
                ui.put(
                    Rect::from_center_size(
                        image_res.rect.center(),
                        Vec2::splat(spinner_size),
                    ),
                    Spinner::new().size(spinner_size),
                );
            }

            if seekbar_hovered || currently_seeking {
                if let Some(hover_pos) = seekbar_response.hover_pos() {
                    if seekbar_response.clicked() || seekbar_response.dragged() {
                        let seek_frac = ((hover_pos - image_res.rect.left_top()).x
                            - seekbar_width_offset / 2.)
                            .max(0.)
                            .min(full_seek_bar_width)
                            / full_seek_bar_width;
                        seekbar_rect.set_right(
                            hover_pos
                                .x
                                .min(full_seek_bar_rect.right())
                                .max(full_seek_bar_rect.left()),
                        );
                        if is_stopped {
                            self.start()
                        }
                        let frame = seek_frac as f64 * self.duration as f64;
                        self.seek(frame as i64);
                    }
                }
            }
            let text_color = Color32::WHITE.linear_multiply(seekbar_anim_frac);

            let pause_icon = if is_paused {
                "▶"
            } else if is_stopped {
                "◼"
            } else if currently_seeking {
                "↔"
            } else {
                "⏸"
            };
            let audio_volume_frac = self.audio_volume.get();
            let sound_icon = if self.get_mute() {
                "🔇"
            } else if audio_volume_frac > 0.7 {
                "🔊"
            } else if audio_volume_frac > 0.4 {
                "🔉"
            } else if audio_volume_frac > 0. {
                "🔈"
            } else {
                "🔇"
            };
            let mut icon_font_id = FontId::default();
            icon_font_id.size = 16.;

            let text_y_offset = -7.;
            let sound_icon_offset = vec2(-5., text_y_offset);
            let sound_icon_pos = full_seek_bar_rect.right_top() + sound_icon_offset;

            let pause_icon_offset = vec2(3., text_y_offset);
            let pause_icon_pos = full_seek_bar_rect.left_top() + pause_icon_offset;

            let duration_text_offset = vec2(25., text_y_offset);
            let duration_text_pos = full_seek_bar_rect.left_top() + duration_text_offset;
            let mut duration_text_font_id = FontId::default();
            duration_text_font_id.size = 14.;

            let mut shadow = Shadow::big_light();
            shadow.color = shadow.color.linear_multiply(seekbar_anim_frac);

            let mut shadow_rect = image_res.rect;
            shadow_rect.set_top(shadow_rect.bottom() - seekbar_offset - 10.);
            let shadow_mesh = shadow.tessellate(shadow_rect, Rounding::ZERO);

            let full_seek_bar_color = Color32::GRAY.linear_multiply(seekbar_anim_frac);
            let seekbar_color = Color32::WHITE.linear_multiply(seekbar_anim_frac);

            ui.painter().add(shadow_mesh);

            ui.painter().rect_filled(
                full_seek_bar_rect,
                Rounding::ZERO,
                full_seek_bar_color.linear_multiply(0.5),
            );
            ui.painter()
                .rect_filled(seekbar_rect, Rounding::ZERO, seekbar_color);
            ui.painter().text(
                pause_icon_pos,
                Align2::LEFT_BOTTOM,
                pause_icon,
                icon_font_id.clone(),
                text_color,
            );

            ui.painter().text(
                duration_text_pos,
                Align2::LEFT_BOTTOM,
                self.duration_text(),
                duration_text_font_id,
                text_color,
            );

            if seekbar_hover_anim_frac > 0. {
                ui.painter().circle_filled(
                    seekbar_rect.right_center(),
                    7. * seekbar_hover_anim_frac,
                    seekbar_color,
                );
            }

            {
                if image_res.clicked() {
                    match self.player_state.get() {
                        PlayerState::Paused => self.player_state.set(PlayerState::Playing),
                        PlayerState::Playing => self.player_state.set(PlayerState::Paused),
                        _ => (),
                    }
                }
                if image_res.double_clicked() {
                    self.command_ui.set(CommandUi::FullscreenToggle);
                }
            }

            {
                let sound_icon_rect = ui.painter().text(
                    sound_icon_pos,
                    Align2::RIGHT_BOTTOM,
                    sound_icon,
                    icon_font_id.clone(),
                    text_color,
                );

                if ui.interact(sound_icon_rect, image_res.id.with("sound_icon_sense"), Sense::click()).clicked() {
                    let mute = self.get_mute();
                    self.set_mute(!mute);
                }
                let mut sound_slider_rect = sound_icon_rect;
                sound_slider_rect.set_bottom(sound_icon_rect.bottom());
                sound_slider_rect.set_top(sound_slider_rect.top() - 124.0);

                let sound_slider_hovered = ui.rect_contains_pointer(sound_slider_rect);
                if sound_slider_hovered {
                    let mut volume = -player::kits::Volume::int_volume(self.audio_volume.get());
                    let mut volume_slider = egui::Slider::new(&mut volume, -player::kits::Volume::MAX_INT_VOLUME..=0).vertical();
                    volume_slider = volume_slider.show_value(false);
                    if ui.put(sound_slider_rect, volume_slider).changed() {
                        let v = player::kits::Volume::f64_volume(-volume);
                        self.audio_volume.set(v);
                    }
                }
            }

            Some(seekbar_interact_rect)
        } else {
            if hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            }
            None
        }
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
    pub fn seek(&mut self, frame_number: i64) {
        self.command_go.set(CommandGo::Seek(frame_number));
    }

    pub fn tab_seek(&mut self) {
        if self.tab_seek_ms > 0 {
            let seek_pos = (self.tab_seek_ms * self.duration) / self.duration_ms;
            self.seek(seek_pos);
        }
    }
    // seek in play ctrl
    pub fn reset(&mut self) {
        self.seek(0);
    }

    /// 此方法最好在 [PlayerState::Paused] 时使用。
    /// 如值为1： 当前是在3号packet frame, 那么它会跳过当前3号，显示4号frame packet,  4-3 = 1。
    /// 如值为-1: 当前是在3号packet frame, 那么它会跳过当前3号，显示2号frame packet。这个功能现在还不支持。
    /// 注： 由于dts(Decoding Time Stamp)与 pts(presentation Time Stamp)是不相同，所以-1的功能实现会有问题，只能通过缓存来解决，内存使用很多，暂时不支持
    // pub fn next_packets(&mut self) {
    //     self.next_packet_frame.set(PacketFrame::Packet);
    // }
    // pub fn next_frames(&mut self) {
    //     self.next_packet_frame.set(PacketFrame::Frame);
    // }

    pub fn go_ahead_ui(&mut self, command_go_ui: &Shared<CommandGo>) {
        self.command_go.set(command_go_ui.get());
    }
    pub fn go_back_ui(&mut self, command_go_ui: &Shared<CommandGo>) {
        match command_go_ui.get() {
            CommandGo::Frame(t) => self.command_go.set(CommandGo::Frame(-t)),
            CommandGo::Packet(t) => self.command_go.set(CommandGo::Packet(-t)),
            CommandGo::GoMs(t) => self.command_go.set(CommandGo::GoMs(-t)),
            CommandGo::None => self.command_go.set(CommandGo::None),
            _ => {}
        }
    }
    pub fn get_mute(&self) -> bool {
        self.audio_dev.get_mute()
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.audio_dev.set_mute(mute);
    }

    // seek in play ctrl
    fn set_state(&mut self, new_state: PlayerState) {
        match new_state {
            PlayerState::Stopped => {
                // self.audio_dev.set_pause(true);
            }
            PlayerState::EndOfFile => {
                // self.audio_dev.set_pause(true);
            }
            PlayerState::Seeking(_) => {}
            PlayerState::Paused => {
                // self.audio_dev.set_pause(true);
            }
            PlayerState::Playing => {
                self.audio_dev.resume();
            }
            PlayerState::Restarting => {
                self.audio_dev.resume();
            }
        }
        self.player_state.set(new_state);
    }

    pub(crate) fn show_seekbar(&self) -> bool {
        let ts = Utc::now().timestamp_millis();
        if ts - self.mouth_move_ts < MAX_DIFF_MOVE_MOUSE {
            return true;
        }
        return false;
    }

    fn duration_frac(&mut self) -> f32 {
        self.elapsed_ms() as f32 / self.duration_ms as f32
    }

    pub fn duration_text(&mut self) -> String {
        format!(
            "{} / {}",
            Self::format_duration(chrono::Duration::milliseconds(self.elapsed_ms())),
            Self::format_duration(chrono::Duration::milliseconds(self.duration_ms))
        )
    }
    fn format_duration(dur: chrono::Duration) -> String {
        let dt = DateTime::<Utc>::from(UNIX_EPOCH) + dur;
        if dt.format("%H").to_string().parse::<i64>().unwrap() > 0 {
            dt.format("%H:%M:%S").to_string()
        } else {
            dt.format("%M:%S").to_string()
        }
    }

    fn process_state(&mut self) {
        let mut reset_stream = false;

        match self.player_state.get() {
            PlayerState::EndOfFile => {
                self.player_state.set(PlayerState::Stopped);
            }
            PlayerState::Stopped => {
                //todo
            }
            PlayerState::Seeking(seek_in_progress) => {
                if self.last_seek_ms.is_some() {
                    // let video_elapsed_ms = self.video_elapsed_ms.get();
                    let last_seek_ms = *self.last_seek_ms.as_ref().unwrap();
                    // if (millisecond_approx_eq(video_elapsed_ms, last_seek_ms) || video_elapsed_ms == 0)
                    if !seek_in_progress {
                        // if let Some(previous_player_state) = self.pre_seek_player_state {
                        //     self.set_state(previous_player_state)
                        // }
                        self.video_elapsed_ms_override.set(-1);
                        self.last_seek_ms = None;
                    } else {
                        self.video_elapsed_ms_override.set(last_seek_ms);
                    }
                } else {
                    self.video_elapsed_ms_override.set(-1);
                }
            }
            PlayerState::Restarting => reset_stream = true,
            _ => (),
        }

        if reset_stream {
            self.reset();
        }
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
        self.stop();
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

#[cfg(not(feature = "meh_ffmpeg"))]
fn print_meda_info(context: &ffmpeg::format::context::Input) -> Result<(), anyhow::Error> {
    for (k, v) in context.metadata().iter() {
        println!("{}: {}", k, v);
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Video) {
        println!("Best video stream index: {}", stream.index());
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Audio) {
        println!("Best audio stream index: {}", stream.index());
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Subtitle) {
        println!("Best subtitle stream index: {}", stream.index());
    }

    println!(
        "duration (seconds): {:.2}",
        context.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE)
    );

    for stream in context.streams() {
        println!("stream index {}:", stream.index());
        println!("\ttime_base: {}", stream.time_base());
        println!("\tstart_time: {}", stream.start_time());
        println!("\tduration (stream timebase): {}", stream.duration());
        println!(
            "\tduration (seconds): {:.2}",
            stream.duration() as f64 * f64::from(stream.time_base())
        );
        println!("\tframes: {}", stream.frames());
        println!("\tdisposition: {:?}", stream.disposition());
        println!("\tdiscard: {:?}", stream.discard());
        println!("\trate: {}", stream.rate());

        let codec = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        println!("\tmedium: {:?}", codec.medium());
        println!("\tid: {:?}", codec.id());

        if codec.medium() == ffmpeg::media::Type::Video {
            if let Ok(video) = codec.decoder().video() {
                println!("\tbit_rate: {}", video.bit_rate());
                println!("\tmax_rate: {}", video.max_bit_rate());
                println!("\tdelay: {}", video.delay());
                println!("\tvideo.width: {}", video.width());
                println!("\tvideo.height: {}", video.height());
                println!("\tvideo.format: {:?}", video.format());
                println!("\tvideo.has_b_frames: {}", video.has_b_frames());
                println!("\tvideo.aspect_ratio: {}", video.aspect_ratio());
                println!("\tvideo.color_space: {:?}", video.color_space());
                println!("\tvideo.color_range: {:?}", video.color_range());
                println!("\tvideo.color_primaries: {:?}", video.color_primaries());
                println!(
                    "\tvideo.color_transfer_characteristic: {:?}",
                    video.color_transfer_characteristic()
                );
                println!("\tvideo.chroma_location: {:?}", video.chroma_location());
                println!("\tvideo.references: {}", video.references());
                println!("\tvideo.intra_dc_precision: {}", video.intra_dc_precision());
            }
        } else if codec.medium() == ffmpeg::media::Type::Audio {
            if let Ok(audio) = codec.decoder().audio() {
                println!("\tbit_rate: {}", audio.bit_rate());
                println!("\tmax_rate: {}", audio.max_bit_rate());
                println!("\tdelay: {}", audio.delay());
                println!("\taudio.rate: {}", audio.rate());
                println!("\taudio.channels: {}", audio.channels());
                println!("\taudio.format: {:?}", audio.format());
                println!("\taudio.frames: {}", audio.frames());
                println!("\taudio.align: {}", audio.align());
                println!("\taudio.channel_layout: {:?}", audio.channel_layout());
            }
        }
    }
    Ok(())
}

#[cfg(feature = "meh_ffmpeg")]
fn print_meda_info(context: &ffmpeg::format::context::Input) -> Result<(), anyhow::Error> {
    for (k, v) in context.metadata().iter() {
        println!("{}: {}", k, v);
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Video) {
        println!("Best video stream index: {}", stream.index());
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Audio) {
        println!("Best audio stream index: {}", stream.index());
    }

    if let Some(stream) = context.streams().best(ffmpeg::media::Type::Subtitle) {
        println!("Best subtitle stream index: {}", stream.index());
    }

    println!(
        "duration (seconds): {:?}",
        context
            .duration()
            .map(|d| d as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE))
    );

    for stream in context.streams() {
        println!("stream index {}:", stream.index());
        println!("\ttime_base: {:?}", stream.time_base());
        println!("\tstart_time: {:?}", stream.start_time());
        println!("\tduration (stream timebase): {:?}", stream.duration());
        println!(
            "\tduration (seconds): {:?}",
            stream
                .time_base()
                .zip(stream.duration())
                .map(|(tb, d)| d as f64 * f64::from(tb))
        );
        println!("\tframes: {}", stream.frames());
        println!("\tdisposition: {:?}", stream.disposition());
        println!("\tdiscard: {:?}", stream.discard());
        println!("\tframe rate: {}", stream.frame_rate());

        let codec_par = stream.parameters();
        println!("\tmedium: {:?}", codec_par.medium());
        println!("\tid: {:?}", codec_par.id());

        let dec = stream.decoder().expect("Unable to open decoder");

        if codec_par.medium() == ffmpeg::media::Type::Video {
            if let Ok(video) = dec.video() {
                println!("\tbit_rate: {}", video.bit_rate());
                println!("\tmax_rate: {}", video.max_bit_rate());
                println!("\tdelay: {}", video.delay());
                println!("\tvideo.width: {}", video.width());
                println!("\tvideo.height: {}", video.height());
                println!("\tvideo.format: {:?}", video.format());
                println!("\tvideo.has_b_frames: {}", video.has_b_frames());
                println!("\tvideo.aspect_ratio: {}", video.aspect_ratio());
                println!("\tvideo.color_space: {:?}", video.color_space());
                println!("\tvideo.color_range: {:?}", video.color_range());
                println!("\tvideo.color_primaries: {:?}", video.color_primaries());
                println!(
                    "\tvideo.color_transfer_characteristic: {:?}",
                    video.color_transfer_characteristic()
                );
                println!("\tvideo.chroma_location: {:?}", video.chroma_location());
                println!("\tvideo.references: {}", video.references());
                println!("\tvideo.intra_dc_precision: {}", video.intra_dc_precision());
            }
        } else if codec_par.medium() == ffmpeg::media::Type::Audio {
            if let Ok(audio) = dec.audio() {
                println!("\tbit_rate: {}", audio.bit_rate());
                println!("\tmax_rate: {}", audio.max_bit_rate());
                println!("\tdelay: {}", audio.delay());
                println!("\taudio.sample_rate: {}", audio.sample_rate());
                println!("\taudio.channels: {}", audio.channels());
                println!("\taudio.format: {:?}", audio.format());
                println!("\taudio.frames: {}", audio.frames());
                println!("\taudio.align: {}", audio.align());
                println!("\taudio.channel_layout: {:?}", audio.channel_layout());
            }
        }
    }
    Ok(())
}

