use ffmpeg::{Rational, rescale, Rescale};

use crate::{is_ffmpeg_eof_error, MILLISEC_TIME_BASE, PlayerState, timestamp_to_millisecond};
use crate::kits::Shared;

/// Streams data.
pub trait Streamer: Send {
    /// The associated type of frame used for the stream.
    type Frame;
    /// The associated type after the frame is processed.
    type ProcessedFrame;
    /// Seek to a location within the stream.
    fn seek(&mut self, seek_frac: f32) {
        let target_ms = (seek_frac as f64 * self.duration_ms() as f64) as i64;
        let seek_completed = millisecond_approx_eq(target_ms, self.elapsed_ms().get());

        // stop seeking near target so we dont waste cpu cycles
        if !seek_completed {
            let elapsed_ms = self.elapsed_ms().clone();
            let currently_behind_target = || elapsed_ms.get() < target_ms;

            let seeking_backwards = target_ms < self.elapsed_ms().get();
            let target_ts = millisecond_to_timestamp(target_ms, rescale::TIME_BASE);
            // let player_state = self.player_state().clone();
            // let still_seeking = || matches!(player_state.get(), PlayerState::Seeking(_));

            if let Err(_) = self.input_context().seek(target_ts, ..target_ts) {
                // dbg!(e); TODO: propagate error
            } else if seek_frac < 0.03 {
                // prevent seek inaccuracy errors near start of stream
                self.player_state().set(PlayerState::Restarting);
                return;
            } else if seek_frac >= 1.0 {
                // disable this safeguard for now (fixed?)
                // prevent infinite loop near end of stream
                self.player_state().set(PlayerState::EndOfFile);
                return;
            } else {
                // this drop frame loop lets us refresh until current_ts is accurate
                if seeking_backwards {
                    while !currently_behind_target() {
                        if let Err(e) = self.drop_frames() {
                            if is_ffmpeg_eof_error(&e) {
                                break;
                            }
                        }
                    }
                }

                // // this drop frame loop drops frames until we are at desired
                while currently_behind_target() {
                    if let Err(e) = self.drop_frames() {
                        if is_ffmpeg_eof_error(&e) {
                            break;
                        }
                    }
                }

                // frame preview
                if self.is_primary_streamer() {
                    match self.receive_next_packet_until_frame() {
                        Ok(frame) => self.apply_frame(frame),
                        _ => (),
                    }
                }
            }
        }
        if self.is_primary_streamer() {
            self.player_state().set(PlayerState::Seeking(false));
        }
    }

    /// The primary streamer will control most of the state/syncing.
    fn is_primary_streamer(&self) -> bool;

    /// The stream index.
    fn stream_index(&self) -> usize;
    /// The elapsed time, in milliseconds.
    fn elapsed_ms(&mut self) -> &mut Shared<i64>;
    /// The total duration of the stream, in milliseconds.
    fn duration_ms(&mut self) -> i64;
    /// The streamer's decoder.
    fn decoder(&mut self) -> &mut ffmpeg::decoder::Opened;
    /// The streamer's input context.
    fn input_context(&mut self) -> &mut ffmpeg::format::context::Input;
    /// The streamer's state.
    fn player_state(&self) -> &Shared<PlayerState>;
    /// Output a frame from the decoder.
    fn decode_frame(&mut self) -> anyhow::Result<Self::Frame>;
    /// Ignore the remainder of this packet.
    fn drop_frames(&mut self) -> anyhow::Result<()> {
        if self.decode_frame().is_err() {
            self.receive_next_packet()
        } else {
            self.drop_frames()
        }
    }
    /// Receive the next packet of the stream.
    fn receive_next_packet(&mut self) -> anyhow::Result<()> {
        if let Some((stream, packet)) = self.input_context().packets().next() {
            let time_base = stream.time_base();
            if stream.index() == self.stream_index() {
                self.decoder().send_packet(&packet)?;
                if let Some(dts) = packet.dts() {
                    self.elapsed_ms().set(timestamp_to_millisecond(dts, time_base));
                }
            }
        } else {
            self.decoder().send_eof()?;
            // self.player_state().set(PlayerState::EndOfFile);
        }
        Ok(())
    }
    /// Reset the stream to its initial state.
    fn reset(&mut self) {
        let beginning: i64 = 0;
        let beginning_seek = beginning.rescale((1, 1), rescale::TIME_BASE);
        let _ = self.input_context().seek(beginning_seek, ..beginning_seek);
        self.decoder().flush();
    }
    /// Keep receiving packets until a frame can be decoded.
    fn receive_next_packet_until_frame(&mut self) -> anyhow::Result<Self::ProcessedFrame> {
        match self.receive_next_frame() {
            Ok(frame_result) => Ok(frame_result),
            Err(e) => {
                if is_ffmpeg_eof_error(&e) {
                    Err(e)
                } else {
                    self.receive_next_packet()?;
                    self.receive_next_packet_until_frame()
                }
            }
        }
    }
    /// Process a decoded frame.
    fn process_frame(&mut self, frame: Self::Frame) -> anyhow::Result<Self::ProcessedFrame>;
    /// Apply a processed frame
    fn apply_frame(&mut self, _frame: Self::ProcessedFrame) {}
    /// Decode and process a frame.
    fn receive_next_frame(&mut self) -> anyhow::Result<Self::ProcessedFrame> {
        match self.decode_frame() {
            Ok(decoded_frame) => self.process_frame(decoded_frame),
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

#[inline(always)]
fn millisecond_approx_eq(a: i64, b: i64) -> bool {
    a.abs_diff(b) < 50
}


fn millisecond_to_timestamp(millisecond: i64, time_base: Rational) -> i64 {
    millisecond.rescale(MILLISEC_TIME_BASE, time_base)
}

