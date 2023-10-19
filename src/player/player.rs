use std::default::Default;
use std::ops::{Deref, DerefMut};
use std::path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};
use egui::{Align2, Color32, FontId, Image, Rect, Response, Rounding, Sense, Spinner, Ui, vec2, Vec2};
use egui::epaint::Shadow;
use egui::load::SizedTexture;
use ffmpeg::{Rational, Rescale, rescale};
use ffmpeg::decoder::Video;
use ffmpeg::software::resampling::Context as ResamplingContext;
use kanal::{Receiver, Sender};

use crate::player::{AV_TIME_BASE_RATIONAL, PlayerState};
use crate::player::audio::{AudioDevice, AudioFrame};
use crate::player::consts::{AUDIO_FRAME_QUEUE_SIZE, AUDIO_PACKET_QUEUE_SIZE, PLAY_MIN_INTERVAL, VIDEO_FRAME_QUEUE_SIZE, VIDEO_PACKET_QUEUE_SIZE};
use crate::player::kits::timestamp_to_millisecond;
use crate::player::play_ctrl::PlayCtrl;
use crate::player::video::VideoFrame;

/// player base ffmpeg, there are 4 threads to player file.
pub struct Player {
    //是否需要停止播放相关线程
    pub play_ctrl: PlayCtrl,
    pub width: u32,
    pub height: u32,

    pub max_audio_volume: f32,
    duration_ms: i64,
    last_seek_ms: Option<i64>,
    // pre_seek_player_state: Option<PlayerState>,
    #[cfg(feature = "from_bytes")]
    temp_file: Option<NamedTempFile>,
    video_elapsed_ms_override: Option<i64>,
}

impl Player {
    //初始化所有线程，如果之前的还在，结束它们
    pub fn new(ctx: &egui::Context, file: &str) -> Result<Player, anyhow::Error> {
        let max_audio_volume = 1.;
        let play_ctrl = {
            let (producer, consumer) = ringbuf::HeapRb::<f32>::new(8192).split();
            let audio_dev = Arc::new(AudioDevice::new(consumer)?);
            PlayCtrl::new(producer, audio_dev, Self::default_texture_handle(ctx))
        };

        //打开文件
        let format_input = ffmpeg::format::input(&path::Path::new(file))?;
        let mut player = Self {
            play_ctrl,
            width: 0,
            height: 0,
            max_audio_volume,
            duration_ms: timestamp_to_millisecond(format_input.duration(), AV_TIME_BASE_RATIONAL),
            last_seek_ms: None,
            video_elapsed_ms_override: None,
        };

        // 获取视频解码器
        let (video_index, video_decoder, video_decoder2) = {
            let video_stream = format_input.streams().best(ffmpeg::media::Type::Video).ok_or(ffmpeg::Error::InvalidData)?;
            let video_index = video_stream.index();
            let video_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
            let video_decoder = video_context.decoder().video()?;
            let video_decoder2 = {
                ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?.decoder().video()?
            };
            player.width = video_decoder.width();
            player.height = video_decoder.height();
            (video_index, video_decoder, video_decoder2)
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
        player.read_packet_run(format_input, ctx.clone(), video_decoder2, audio_packet_sender, audio_index,
                               video_packet_sender, video_index);

        // player.play_ctrl.set_pause(false);
        Ok(player)
    }

    pub fn default_texture_handle(ctx: &egui::Context) -> egui::TextureHandle {
        let texture_options = egui::TextureOptions::LINEAR;
        let texture_handle = ctx.load_texture("video_stream_default", egui::ColorImage::example(), texture_options);
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

    fn audio_decode_run(&self, mut audio_decoder: ffmpeg::decoder::Audio, packet_receiver: Receiver<Option<ffmpeg::Packet>>, audio_deque: Sender<AudioFrame>) {
        let play_ctrl = self.play_ctrl.clone();
        let mut audio_re_sampler = {
            let stream_config = play_ctrl.audio_config();
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
        let _ = std::thread::Builder::new().name("audio decode".to_string()).spawn(move || {
            'RUN: loop {
                if PlayerState::Stopped == play_ctrl.player_state.get() {
                    break 'RUN;
                }

                loop {
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
                            let pts = frame_old.pts().expect("") as f64 / frame_old.rate() as f64;
                            let duration = frame_old.samples() as f64 / frame_old.rate() as f64;
                            let v = play_ctrl.audio_volume.get();
                            let samples: Vec<f32> = re_samples_ref.iter().map(|s| s * v).collect();
                            let audio_frame = AudioFrame {
                                samples,
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
                            log::debug!("{}", e);
                            break;
                        }
                    }
                    if PlayerState::Stopped == play_ctrl.player_state.get() {
                        break 'RUN;
                    }
                }

                match packet_receiver.recv() {
                    Err(e) => {
                        log::error!("{}", e);
                        break 'RUN;
                    }
                    Ok(Some(packet)) => {
                        if PlayerState::Stopped == play_ctrl.player_state.get() {
                            break 'RUN;
                        }
                        match audio_decoder.0.send_packet(&packet) {
                            Err(e) => {
                                log::error!("{}", e);
                            }
                            Ok(_) => {}
                        }
                    }
                    Ok(None) => {
                        // match audio_decoder.0.send_eof() {
                        //     Err(e) => {
                        //         log::error!("{}", e);
                        //     }
                        //     Ok(_) => {}
                        // }
                        // audio_decoder.flush();
                    }
                }
            }
        });
    }

    fn audio_play_run(&self, frame_deque: Receiver<AudioFrame>) {
        let mut play_ctrl = self.play_ctrl.clone();
        let _ = std::thread::Builder::new().name("audio play".to_string()).spawn(move || {
            let mut empty_count = 0;
            loop {
                if play_ctrl.player_state.get() == PlayerState::Stopped {
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
                    if play_ctrl.player_state.get() == PlayerState::Paused {
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

    fn video_decode_run(&self, mut video_decoder: ffmpeg::decoder::Video, packet_receiver: kanal::Receiver<Option<ffmpeg::Packet>>, video_deque: Sender<VideoFrame>, _: Rational) {
        let play_ctrl = self.play_ctrl.clone();
        let width = video_decoder.width() as usize;
        let height = video_decoder.height() as usize;

        // let duration = 1.0 / av_q2d(video_decoder..framerate);
        let _ = std::thread::Builder::new().name("video decode".to_string()).spawn(move || loop {
            if PlayerState::Stopped == play_ctrl.player_state.get() {
                break;
            }
            let mut frame = ffmpeg::frame::Video::empty();
            match video_decoder.receive_frame(&mut frame) {
                Err(e) => {
                    log::debug!("{}", e);
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
                Ok(Some(packet)) => {
                    match video_decoder.0.send_packet(&packet) {
                        Err(e) => {
                            log::error!("{}", e);
                        }
                        Ok(_) => {}
                    }
                }
                Ok(None) => {
                    // match video_decoder.0.send_eof() {
                    //     Err(e) => {
                    //         log::error!("{}", e);
                    //     }
                    //     Ok(_) => {}
                    // }
                    // video_decoder.flush();
                }
            }
        });
    }

    fn video_play_run(&self, ctx: egui::Context, frame_deque: Receiver<VideoFrame>) {
        let mut play_ctrl = self.play_ctrl.clone();
        let _ = std::thread::Builder::new().name("video play".to_string()).spawn(move || {
            let mut empty_count = 0;
            loop {
                if PlayerState::Stopped == play_ctrl.player_state.get() {
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
                    if play_ctrl.player_state.get() == PlayerState::Paused {
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

    fn read_packet_run(&self, mut input: ffmpeg::format::context::Input,
                       ctx: egui::Context,
                       mut video_decoder: Video, audio_deque: kanal::Sender<Option<ffmpeg::Packet>>, audio_index: usize,
                       video_deque: kanal::Sender<Option<ffmpeg::Packet>>, video_index: usize) {
        let mut play_ctrl = self.play_ctrl.clone();
        let duration = input.duration();
        let _ = std::thread::Builder::new().name("read packet".to_string()).spawn(move || {
            //get first video frame, and refresh window
            loop {
                if let Some((_, packet)) = input.packets().next() {
                    if unsafe { packet.is_empty() || packet.stream() != video_index } {
                        continue;
                    }

                    if let Err(e) = video_decoder.0.send_packet(&packet) {
                        log::error!("{}", e);
                        break;
                    }
                    let mut frame = ffmpeg::frame::Video::empty();
                    match video_decoder.receive_frame(&mut frame) {
                        Err(e) => {
                            log::debug!("{}", e);
                        }
                        Ok(_) => {
                            let color_image = match Self::frame_to_color_image(&frame) {
                                Err(e) => {
                                    log::error!("{}", e);
                                    break;
                                }
                                Ok(t) => t,
                            };
                            play_ctrl.texture_handle.set(color_image, egui::TextureOptions::LINEAR);
                            ctx.request_repaint();
                            break;
                        }
                    }
                }
            }
            {
                let beginning: i64 = 0;
                let beginning_seek = beginning.rescale((1, 1), rescale::TIME_BASE);
                let _ = input.seek(beginning_seek, ..beginning_seek);
                video_decoder.flush();
                drop(video_decoder);
                drop(ctx);
            }

            loop {
                if play_ctrl.player_state.get() == PlayerState::Stopped {
                    break;
                }

                if play_ctrl.audio_finished() && play_ctrl.video_finished() {
                    play_ctrl.player_state.set(PlayerState::Stopped);
                    break;
                }

                let scale = play_ctrl.seek_scale.load(Ordering::Relaxed);
                if scale > 0.0 {
                    let seek_pos = (scale * duration as f64) as i64;
                    if let Err(e) = input.seek(seek_pos, ..seek_pos) {
                        log::error!("{}", e);
                    }
                    play_ctrl.seek(-1.0);
                    //清空之前的数据
                    let _ = audio_deque.send(None);
                    let _ = audio_deque.send(None);
                    let _ = video_deque.send(None);
                    let _ = video_deque.send(None);
                } else {
                    if play_ctrl.player_state.get() == PlayerState::Paused || audio_deque.is_full() || video_deque.is_full() {
                        spin_sleep::sleep(PLAY_MIN_INTERVAL);
                        continue;
                    }
                }

                if let Some((stream, packet)) = input.packets().next() {
                    if unsafe { !packet.is_empty() } {
                        if packet.stream() == audio_index {
                            if let Some(dts) = packet.dts() {
                                play_ctrl.audio_elapsed_ms.set(timestamp_to_millisecond(dts, stream.time_base()));
                            }
                            if let Err(e) = audio_deque.send(Some(packet)) {
                                log::error!("{}", e);
                            }
                        } else if packet.stream() == video_index {
                            if let Some(dts) = packet.dts() {
                                play_ctrl.video_elapsed_ms.set(timestamp_to_millisecond(dts, stream.time_base()));
                            }
                            if let Err(e) = video_deque.send(Some(packet)) {
                                log::error!("{}", e);
                            }
                        }
                    }
                } else {
                    play_ctrl.set_packet_finished(true);
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
        self.render_ui(ui, &response);
        self.process_state();
        response
    }
    fn render_ui(&mut self, ui: &mut Ui, image_res: &Response) -> Option<Rect> {
        let hovered = ui.rect_contains_pointer(image_res.rect);
        let currently_seeking = matches!(self.player_state.get(), PlayerState::Seeking(_));
        let is_stopped = matches!(self.player_state.get(), PlayerState::Stopped);
        let is_paused = matches!(self.player_state.get(), PlayerState::Paused);
        let seekbar_anim_frac = ui.ctx().animate_bool_with_time(
            image_res.id.with("seekbar_anim"),
            hovered || currently_seeking || is_paused || is_stopped,
            0.2,
        );

        if seekbar_anim_frac > 0. {
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
                        self.seek(seek_frac as f64);
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
            let audio_volume_frac = self.audio_volume.get() / self.max_audio_volume;
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

            if image_res.clicked() {
                let mut reset_stream = false;
                let mut start_stream = false;

                match self.player_state.get() {
                    PlayerState::Stopped => start_stream = true,
                    PlayerState::EndOfFile => reset_stream = true,
                    PlayerState::Paused => self.player_state.set(PlayerState::Playing),
                    PlayerState::Playing => self.player_state.set(PlayerState::Paused),
                    _ => (),
                }

                if reset_stream {
                    self.reset();
                    self.resume();
                } else if start_stream {
                    self.start();
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

                if ui
                    .interact(
                        sound_icon_rect,
                        image_res.id.with("sound_icon_sense"),
                        Sense::click(),
                    )
                    .clicked()
                {
                    let mute = self.get_mute();
                    self.set_mute(!mute);
                }

                let sound_slider_outer_height = 75.;
                let sound_slider_margin = 5.;
                let sound_slider_opacity = 100;
                let mut sound_slider_rect = sound_icon_rect;
                sound_slider_rect.set_bottom(sound_icon_rect.top() - sound_slider_margin);
                sound_slider_rect.set_top(sound_slider_rect.top() - sound_slider_outer_height);

                let sound_slider_interact_rect = sound_slider_rect.expand(sound_slider_margin);
                let sound_hovered = ui.rect_contains_pointer(sound_icon_rect);
                let sound_slider_hovered = ui.rect_contains_pointer(sound_slider_interact_rect);
                let sound_anim_id = image_res.id.with("sound_anim");
                let mut sound_anim_frac: f32 = ui
                    .ctx()
                    .memory_mut(|m| *m.data.get_temp_mut_or_default(sound_anim_id));
                sound_anim_frac = ui.ctx().animate_bool_with_time(
                    sound_anim_id,
                    sound_hovered || (sound_slider_hovered && sound_anim_frac > 0.),
                    0.2,
                );
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(sound_anim_id, sound_anim_frac));
                let sound_slider_bg_color = Color32::from_black_alpha(sound_slider_opacity)
                    .linear_multiply(sound_anim_frac);
                let sound_bar_color = Color32::from_white_alpha(sound_slider_opacity)
                    .linear_multiply(sound_anim_frac);
                let mut sound_bar_rect = sound_slider_rect;
                sound_bar_rect.set_top(
                    sound_bar_rect.bottom()
                        - (self.audio_volume.get() / self.max_audio_volume)
                        * sound_bar_rect.height(),
                );

                ui.painter().rect_filled(
                    sound_slider_rect,
                    Rounding::same(5.),
                    sound_slider_bg_color,
                );

                ui.painter()
                    .rect_filled(sound_bar_rect, Rounding::same(5.), sound_bar_color);
                let sound_slider_resp = ui.interact(
                    sound_slider_rect,
                    image_res.id.with("sound_slider_sense"),
                    Sense::click_and_drag(),
                );
                if sound_anim_frac > 0. && sound_slider_resp.clicked()
                    || sound_slider_resp.dragged()
                {
                    if let Some(hover_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
                        let sound_frac = 1.
                            - ((hover_pos - sound_slider_rect.left_top()).y
                            / sound_slider_rect.height())
                            .max(0.)
                            .min(1.);
                        self.audio_volume.set(sound_frac * self.max_audio_volume);
                    }
                }
            }

            Some(seekbar_interact_rect)
        } else {
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

    pub fn reset(&mut self) {
        self.seek(0.0);
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
                self.audio_dev.set_pause(true);
            }
            PlayerState::EndOfFile => {
                self.audio_dev.set_pause(true);
            }
            PlayerState::Seeking(_) => {}
            PlayerState::Paused => {
                self.audio_dev.set_pause(true);
            }
            PlayerState::Playing => {
                self.audio_dev.set_pause(false);
            }
            PlayerState::Restarting => {
                self.audio_dev.set_pause(false);
            }
        }
        self.player_state.set(new_state);
    }


    fn elapsed_ms(&self) -> i64 {
        self.video_elapsed_ms_override
            .as_ref()
            .map(|i| *i)
            .unwrap_or(self.video_elapsed_ms.get())
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
                        self.video_elapsed_ms_override = None;
                        self.last_seek_ms = None;
                    } else {
                        self.video_elapsed_ms_override = Some(last_seek_ms);
                    }
                } else {
                    self.video_elapsed_ms_override = None;
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

