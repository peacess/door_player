extern crate ffmpeg_the_third as ffmpeg;

use std::sync::{Arc, Weak};
use std::time::UNIX_EPOCH;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use egui::{self,
           Align2, Color32, ColorImage, epaint::Shadow, FontId, Image, Rect, Response, Rounding, Sense, Spinner,
           TextureHandle, TextureOptions, Ui, vec2,
           Vec2,
};
use ffmpeg::ChannelLayout;
use ffmpeg::format::input;
use ffmpeg::media::Type;
use parking_lot::Mutex;
use ringbuf::SharedRb;
use sdl2::audio::{AudioCallback, AudioFormat, AudioSpecDesired};
use timer::{Guard, Timer};

use crate::{AudioDevice, AudioSampleConsumer, AudioStreamer, AV_TIME_BASE_RATIONAL, is_ffmpeg_eof_error, PlayerState, VideoStreamer};
use crate::kits::Shared;
use crate::player::{Streamer, timestamp_to_millisecond};

fn format_duration(dur: Duration) -> String {
    let dt = DateTime::<Utc>::from(UNIX_EPOCH) + dur;
    if dt.format("%H").to_string().parse::<i64>().unwrap() > 0 {
        dt.format("%H:%M:%S").to_string()
    } else {
        dt.format("%M:%S").to_string()
    }
}


/// The [`Player`] processes and controls streams of video/audio. This is what you use to show a video file.
/// Initialize once, and use the [`Player::ui`] or [`Player::ui_at()`] functions to show the playback.
pub struct Player {
    /// The video streamer of the player.
    pub video_streamer: Arc<Mutex<VideoStreamer>>,
    /// The audio streamer of the player. Won't exist unless [`Player::with_audio`] is called and there exists
    /// a valid audio stream in the file.
    pub audio_streamer: Option<Arc<Mutex<AudioStreamer>>>,
    /// The state of the player.
    pub player_state: Shared<PlayerState>,
    /// The frame rate of the video stream.
    pub frame_rate: f64,
    texture_options: TextureOptions,
    /// The player's texture handle.
    pub texture_handle: TextureHandle,
    /// The height of the video stream.
    pub height: u32,
    /// The width of the video stream.
    pub width: u32,
    frame_timer: Timer,
    audio_timer: Timer,
    audio_thread: Option<Guard>,
    frame_thread: Option<Guard>,
    ctx_ref: egui::Context,
    /// Should the stream loop if it finishes?
    pub looping: bool,
    /// The volume of the audio stream.
    pub audio_volume: Shared<f32>,
    /// The maximum volume of the audio stream.
    pub max_audio_volume: f32,
    duration_ms: i64,
    last_seek_ms: Option<i64>,
    pre_seek_player_state: Option<PlayerState>,
    #[cfg(feature = "from_bytes")]
    temp_file: Option<NamedTempFile>,
    video_elapsed_ms: Shared<i64>,
    audio_elapsed_ms: Shared<i64>,
    video_elapsed_ms_override: Option<i64>,
    input_path: String,
}


impl Player {
    /// A formatted string for displaying the duration of the video stream.
    pub fn duration_text(&mut self) -> String {
        format!(
            "{} / {}",
            format_duration(Duration::milliseconds(self.elapsed_ms())),
            format_duration(Duration::milliseconds(self.duration_ms))
        )
    }
    fn reset(&mut self) {
        self.last_seek_ms = None;
        self.video_elapsed_ms_override = None;
        self.video_elapsed_ms.set(0);
        self.audio_elapsed_ms.set(0);
        self.video_streamer.lock().reset();
        if let Some(audio_decoder) = self.audio_streamer.as_mut() {
            audio_decoder.lock().reset();
        }
    }
    fn elapsed_ms(&self) -> i64 {
        self.video_elapsed_ms_override
            .as_ref()
            .map(|i| *i)
            .unwrap_or(self.video_elapsed_ms.get())
    }
    fn set_state(&mut self, new_state: PlayerState) {
        self.player_state.set(new_state)
    }
    /// Pause the stream.
    pub fn pause(&mut self) {
        self.set_state(PlayerState::Paused)
    }
    /// Resume the stream from a paused state.
    pub fn resume(&mut self) {
        self.set_state(PlayerState::Playing)
    }
    /// Stop the stream.
    pub fn stop(&mut self) {
        self.set_state(PlayerState::Stopped)
    }
    /// Directly stop the stream. Use if you need to immediately end the streams, and/or you
    /// aren't able to call the player's [`Player::ui`]/[`Player::ui_at`] functions later on.
    pub fn stop_direct(&mut self) {
        self.frame_thread = None;
        self.audio_thread = None;
        self.reset()
    }
    fn duration_frac(&mut self) -> f32 {
        self.elapsed_ms() as f32 / self.duration_ms as f32
    }
    /// Seek to a location in the stream.
    pub fn seek(&mut self, seek_frac: f32) {
        let current_state = self.player_state.get();
        if !matches!(current_state, PlayerState::Seeking(true)) {
            match current_state {
                PlayerState::Stopped | PlayerState::EndOfFile => {
                    self.pre_seek_player_state = Some(PlayerState::Paused);
                    self.start();
                }
                PlayerState::Paused | PlayerState::Playing => {
                    self.pre_seek_player_state = Some(current_state);
                }
                _ => (),
            }

            let video_streamer = self.video_streamer.clone();

            if let Some(audio_streamer) = self.audio_streamer.as_mut() {
                audio_streamer.lock().seek(seek_frac);
            };

            self.last_seek_ms = Some((seek_frac as f64 * self.duration_ms as f64) as i64);
            self.set_state(PlayerState::Seeking(true));

            std::thread::spawn(move || {
                video_streamer.lock().seek(seek_frac);
            });
        }
    }
    fn spawn_timers(&mut self) {
        let mut texture_handle = self.texture_handle.clone();
        let texture_options = self.texture_options.clone();
        let ctx = self.ctx_ref.clone();
        let wait_duration = Duration::milliseconds((1000. / self.frame_rate) as i64);

        fn play<T: Streamer>(streamer: &Weak<Mutex<T>>) {
            if let Some(streamer) = streamer.upgrade() {
                if let Some(mut streamer) = streamer.try_lock() {
                    if streamer.player_state().get() == PlayerState::Playing {
                        match streamer.receive_next_packet_until_frame() {
                            Ok(frame) => streamer.apply_frame(frame),
                            Err(e) => {
                                if is_ffmpeg_eof_error(&e) && streamer.is_primary_streamer() {
                                    streamer.player_state().set(PlayerState::EndOfFile)
                                }
                            }
                        }
                    }
                }
            }
        }

        self.video_streamer.lock().apply_video_frame_fn = Some(Box::new(move |frame| {
            texture_handle.set(frame, texture_options)
        }));

        let video_streamer_ref = Arc::downgrade(&self.video_streamer);

        let frame_timer_guard = self.frame_timer.schedule_repeating(wait_duration, move || {
            play(&video_streamer_ref);
            ctx.request_repaint();
        });

        self.frame_thread = Some(frame_timer_guard);

        if let Some(audio_decoder) = self.audio_streamer.as_ref() {
            let audio_decoder_ref = Arc::downgrade(&audio_decoder);
            let audio_timer_guard = self
                .audio_timer
                .schedule_repeating(Duration::zero(), move || play(&audio_decoder_ref));
            self.audio_thread = Some(audio_timer_guard);
        }
    }
    /// Start the stream.
    pub fn start(&mut self) {
        self.stop_direct();
        self.spawn_timers();
        self.resume();
    }
    fn process_state(&mut self) {
        let mut reset_stream = false;

        match self.player_state.get() {
            PlayerState::EndOfFile => {
                if self.looping {
                    reset_stream = true;
                } else {
                    self.player_state.set(PlayerState::Stopped);
                }
            }
            PlayerState::Stopped => {
                self.stop_direct();
            }
            PlayerState::Seeking(seek_in_progress) => {
                if self.last_seek_ms.is_some() {
                    // let video_elapsed_ms = self.video_elapsed_ms.get();
                    let last_seek_ms = *self.last_seek_ms.as_ref().unwrap();
                    // if (millisecond_approx_eq(video_elapsed_ms, last_seek_ms) || video_elapsed_ms == 0)
                    if !seek_in_progress {
                        if let Some(previous_player_state) = self.pre_seek_player_state {
                            self.set_state(previous_player_state)
                        }
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
            self.resume();
        }
    }

    /// Draw the player's ui and process state changes.
    pub fn ui(&mut self, ui: &mut Ui, size: [f32; 2]) -> Response {
        let image = Image::new(self.texture_handle.id(), size).sense(Sense::click());
        let response = ui.add(image);
        self.render_ui(ui, &response);
        self.process_state();
        response
    }

    /// Draw the player's ui with a specific rect, and process state changes.
    pub fn ui_at(&mut self, ui: &mut Ui, rect: Rect) -> Response {
        let image = Image::new(self.texture_handle.id(), rect.size()).sense(Sense::click());
        let response = ui.put(rect, image);
        self.render_ui(ui, &response);
        self.process_state();
        response
    }

    fn render_ui(&mut self, ui: &mut Ui, playback_response: &Response) -> Option<Rect> {
        let hovered = ui.rect_contains_pointer(playback_response.rect);
        let currently_seeking = matches!(self.player_state.get(), PlayerState::Seeking(_));
        let is_stopped = matches!(self.player_state.get(), PlayerState::Stopped);
        let is_paused = matches!(self.player_state.get(), PlayerState::Paused);
        let seekbar_anim_frac = ui.ctx().animate_bool_with_time(
            playback_response.id.with("seekbar_anim"),
            hovered || currently_seeking || is_paused || is_stopped,
            0.2,
        );

        if seekbar_anim_frac > 0. {
            let seekbar_width_offset = 20.;
            let full_seek_bar_width = playback_response.rect.width() - seekbar_width_offset;

            let seekbar_width = full_seek_bar_width * self.duration_frac();

            let seekbar_offset = 20.;
            let seekbar_pos = playback_response.rect.left_bottom()
                + vec2(seekbar_width_offset / 2., -seekbar_offset);
            let seekbar_height = 3.;
            let mut full_seek_bar_rect =
                Rect::from_min_size(seekbar_pos, vec2(full_seek_bar_width, seekbar_height));

            let mut seekbar_rect =
                Rect::from_min_size(seekbar_pos, vec2(seekbar_width, seekbar_height));
            let seekbar_interact_rect = full_seek_bar_rect.expand(10.);
            ui.interact(seekbar_interact_rect, playback_response.id, Sense::drag());

            let seekbar_response = ui.interact(
                seekbar_interact_rect,
                playback_response.id.with("seekbar"),
                Sense::click_and_drag(),
            );

            let seekbar_hovered = seekbar_response.hovered();
            let seekbar_hover_anim_frac = ui.ctx().animate_bool_with_time(
                playback_response.id.with("seekbar_hover_anim"),
                seekbar_hovered || currently_seeking,
                0.2,
            );

            if seekbar_hover_anim_frac > 0. {
                let new_top = full_seek_bar_rect.top() - (3. * seekbar_hover_anim_frac);
                full_seek_bar_rect.set_top(new_top);
                seekbar_rect.set_top(new_top);
            }

            let seek_indicator_anim = ui.ctx().animate_bool_with_time(
                playback_response.id.with("seek_indicator_anim"),
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
                    seek_indicator_shadow.tessellate(playback_response.rect, Rounding::none()),
                );
                ui.put(
                    Rect::from_center_size(
                        playback_response.rect.center(),
                        Vec2::splat(spinner_size),
                    ),
                    Spinner::new().size(spinner_size),
                );
            }

            if seekbar_hovered || currently_seeking {
                if let Some(hover_pos) = seekbar_response.hover_pos() {
                    if seekbar_response.clicked() || seekbar_response.dragged() {
                        let seek_frac = ((hover_pos - playback_response.rect.left_top()).x
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
                        self.seek(seek_frac);
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
            let sound_icon = if audio_volume_frac > 0.7 {
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

            let mut shadow_rect = playback_response.rect;
            shadow_rect.set_top(shadow_rect.bottom() - seekbar_offset - 10.);
            let shadow_mesh = shadow.tessellate(shadow_rect, Rounding::none());

            let full_seek_bar_color = Color32::GRAY.linear_multiply(seekbar_anim_frac);
            let seekbar_color = Color32::WHITE.linear_multiply(seekbar_anim_frac);

            ui.painter().add(shadow_mesh);

            ui.painter().rect_filled(
                full_seek_bar_rect,
                Rounding::none(),
                full_seek_bar_color.linear_multiply(0.5),
            );
            ui.painter()
                .rect_filled(seekbar_rect, Rounding::none(), seekbar_color);
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

            if playback_response.clicked() {
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

            if self.audio_streamer.is_some() {
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
                        playback_response.id.with("sound_icon_sense"),
                        Sense::click(),
                    )
                    .clicked()
                {
                    if self.audio_volume.get() != 0. {
                        self.audio_volume.set(0.)
                    } else {
                        self.audio_volume.set(self.max_audio_volume / 2.)
                    }
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
                let sound_anim_id = playback_response.id.with("sound_anim");
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
                    playback_response.id.with("sound_slider_sense"),
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

    /// Initializes the audio stream (if there is one), required for making a [`Player`] output audio.
    /// Will stop and reset the player's state.
    pub fn set_audio(&mut self, audio_device: &mut AudioDevice) -> Result<()> {
        let audio_input_context = input(&self.input_path)?;
        let audio_stream = audio_input_context.streams().best(Type::Audio);

        let audio_streamer = if let Some(audio_stream) = audio_stream.as_ref() {
            let audio_stream_index = audio_stream.index();
            let audio_context =
                ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())?;
            let audio_decoder = audio_context.decoder().audio()?;
            let audio_sample_buffer =
                SharedRb::<f32, Vec<_>>::new(audio_device.spec().size as usize);
            let (audio_sample_producer, audio_sample_consumer) = audio_sample_buffer.split();
            let audio_re_sampler = ffmpeg::software::resampling::context::Context::get(
                audio_decoder.format(),
                audio_decoder.channel_layout(),
                audio_decoder.rate(),
                audio_device.spec().format.to_sample(),
                ChannelLayout::STEREO,
                audio_device.spec().freq as u32,
            )?;

            audio_device.lock().sample_streams.push(AudioSampleStream {
                sample_consumer: audio_sample_consumer,
                audio_volume: self.audio_volume.clone(),
            });

            audio_device.resume();

            self.stop_direct();

            Some(AudioStreamer {
                duration_ms: self.duration_ms,
                player_state: self.player_state.clone(),
                _video_elapsed_ms: self.video_elapsed_ms.clone(),
                audio_elapsed_ms: self.audio_elapsed_ms.clone(),
                audio_sample_producer,
                input_context: audio_input_context,
                audio_decoder,
                audio_stream_index,
                re_sampler: audio_re_sampler,
            })
        } else {
            None
        };
        self.audio_streamer = audio_streamer.map(|s| Arc::new(Mutex::new(s)));
        Ok(())
    }

    /// Enables using [`Player::set_audio`] with the builder pattern.
    pub fn with_audio(mut self, audio_device: &mut AudioDevice) -> Result<Self> {
        self.set_audio(audio_device)?;
        Ok(self)
    }

    /// Create a new [`Player`].
    pub fn new(ctx: &egui::Context, input_path: &String) -> Result<Self> {
        let input_context = input(&input_path)?;
        let video_stream = input_context
            .streams()
            .best(Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = video_stream.index();
        let max_audio_volume = 1.;

        let audio_volume = Shared::new(max_audio_volume / 2.);

        let video_elapsed_ms = Shared::new(0);
        let audio_elapsed_ms = Shared::new(0);
        let player_state = Shared::new(PlayerState::Stopped);

        let video_context =
            ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;
        let video_decoder = video_context.decoder().video()?;
        let frame_rate = (video_stream.avg_frame_rate().numerator() as f64)
            / video_stream.avg_frame_rate().denominator() as f64;

        let (width, height) = (video_decoder.width(), video_decoder.height());
        let duration_ms = timestamp_to_millisecond(input_context.duration(), AV_TIME_BASE_RATIONAL); // in sec

        let stream_decoder = VideoStreamer {
            apply_video_frame_fn: None,
            duration_ms,
            video_decoder,
            video_stream_index,
            _audio_elapsed_ms: audio_elapsed_ms.clone(),
            video_elapsed_ms: video_elapsed_ms.clone(),
            input_context,
            player_state: player_state.clone(),
        };
        let texture_options = TextureOptions::LINEAR;
        let texture_handle = ctx.load_texture("vidstream", ColorImage::example(), texture_options);
        let mut streamer = Self {
            input_path: input_path.clone(),
            audio_streamer: None,
            video_streamer: Arc::new(Mutex::new(stream_decoder)),
            texture_options,
            frame_rate,
            frame_timer: Timer::new(),
            audio_timer: Timer::new(),
            pre_seek_player_state: None,
            frame_thread: None,
            audio_thread: None,
            texture_handle,
            player_state,
            video_elapsed_ms,
            audio_elapsed_ms,
            width,
            last_seek_ms: None,
            duration_ms,
            audio_volume,
            max_audio_volume,
            video_elapsed_ms_override: None,
            looping: true,
            height,
            ctx_ref: ctx.clone(),
            #[cfg(feature = "from_bytes")]
            temp_file: None,
        };

        loop {
            if let Ok(_texture_handle) = streamer.try_set_texture_handle() {
                break;
            }
        }

        Ok(streamer)
    }

    fn try_set_texture_handle(&mut self) -> Result<TextureHandle> {
        match self.video_streamer.lock().receive_next_packet_until_frame() {
            Ok(first_frame) => {
                let texture_handle =
                    self.ctx_ref
                        .load_texture("vidstream", first_frame, self.texture_options);
                let texture_handle_clone = texture_handle.clone();
                self.texture_handle = texture_handle;
                Ok(texture_handle_clone)
            }
            Err(e) => Err(e),
        }
    }
}


type FfmpegAudioFormat = ffmpeg::format::Sample;
type FfmpegAudioFormatType = ffmpeg::format::sample::Type;

trait AsFfmpegSample {
    fn to_sample(&self) -> ffmpeg::format::Sample;
}

impl AsFfmpegSample for AudioFormat {
    fn to_sample(&self) -> FfmpegAudioFormat {
        match self {
            AudioFormat::U8 => FfmpegAudioFormat::U8(FfmpegAudioFormatType::Packed),
            AudioFormat::S8 => panic!("unsupported audio format"),
            AudioFormat::U16LSB => panic!("unsupported audio format"),
            AudioFormat::U16MSB => panic!("unsupported audio format"),
            AudioFormat::S16LSB => FfmpegAudioFormat::I16(FfmpegAudioFormatType::Packed),
            AudioFormat::S16MSB => FfmpegAudioFormat::I16(FfmpegAudioFormatType::Packed),
            AudioFormat::S32LSB => FfmpegAudioFormat::I32(FfmpegAudioFormatType::Packed),
            AudioFormat::S32MSB => FfmpegAudioFormat::I32(FfmpegAudioFormatType::Packed),
            AudioFormat::F32LSB => FfmpegAudioFormat::F32(FfmpegAudioFormatType::Packed),
            AudioFormat::F32MSB => FfmpegAudioFormat::F32(FfmpegAudioFormatType::Packed),
        }
    }
}

/// Create a new [`AudioDeviceCallback`] from an existing [`sdl2::AudioSubsystem`]. An [`AudioDevice`] is required for using audio.
pub fn init_audio_device(audio_sys: &sdl2::AudioSubsystem) -> Result<AudioDevice, String> {
    AudioDeviceCallback::init(audio_sys)
}

/// Create a new [`AudioDeviceCallback`]. Creates an [`sdl2::AudioSubsystem`]. An [`AudioDevice`] is required for using audio.
pub fn init_audio_device_default() -> Result<AudioDevice, String> {
    AudioDeviceCallback::init(&sdl2::init()?.audio()?)
}

/// Pipes audio samples to SDL2.
pub struct AudioDeviceCallback {
    sample_streams: Vec<AudioSampleStream>,
}

struct AudioSampleStream {
    sample_consumer: AudioSampleConsumer,
    audio_volume: Shared<f32>,
}

impl AudioCallback for AudioDeviceCallback {
    type Channel = f32;
    fn callback(&mut self, output: &mut [Self::Channel]) {
        for x in output.iter_mut() {
            *x = self
                .sample_streams
                .iter_mut()
                .map(|s| s.sample_consumer.pop().unwrap_or(0.) * s.audio_volume.get())
                .sum()
        }
    }
}

impl AudioDeviceCallback {
    fn init(audio_sys: &sdl2::AudioSubsystem) -> Result<AudioDevice, String> {
        let audio_spec = AudioSpecDesired {
            freq: Some(44_100),
            channels: Some(2),
            samples: None,
        };
        let device = audio_sys.open_playback(None, &audio_spec, |_spec| AudioDeviceCallback {
            sample_streams: vec![],
        })?;
        Ok(device)
    }
}




