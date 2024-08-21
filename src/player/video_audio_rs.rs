use crate::player::{AudioPlayFrame, VideoPlayFrame};

#[derive(Clone, Default)]
pub struct VideoAudioRS {
    pub video_packet_receiver: Option<kanal::Receiver<Option<ffmpeg::Packet>>>,
    pub video_packet_sender: Option<kanal::Sender<Option<ffmpeg::Packet>>>,
    pub video_play_receiver: Option<kanal::Receiver<VideoPlayFrame>>,
    pub video_play_sender: Option<kanal::Sender<VideoPlayFrame>>,
    pub video_stream_time_base: Option<ffmpeg::Rational>,

    pub audio_packet_receiver: Option<kanal::Receiver<Option<ffmpeg::Packet>>>,
    pub audio_packet_sender: Option<kanal::Sender<Option<ffmpeg::Packet>>>,
    pub audio_play_receiver: Option<kanal::Receiver<AudioPlayFrame>>,
    pub audio_play_sender: Option<kanal::Sender<AudioPlayFrame>>,
    pub audio_stream_time_base: Option<ffmpeg::Rational>,
}

impl VideoAudioRS {
    pub fn seek_clean(&self) {
        if let Some(receiver) = &self.video_packet_receiver {
            let size = receiver.len();
            for _ in 0..size {
                let _ = receiver.try_recv();
            }
        }
        if let Some(receiver) = &self.video_play_receiver {
            let size = receiver.len();
            for _ in 0..size {
                let _ = receiver.try_recv();
            }
        }
        if let Some(receiver) = &self.audio_packet_receiver {
            let size = receiver.len();
            for _ in 0..size {
                let _ = receiver.try_recv();
            }
        }
        if let Some(receiver) = &self.audio_play_receiver {
            let size = receiver.len();
            for _ in 0..size {
                let _ = receiver.try_recv();
            }
        }
        //clear the video and audio frame
    }
}
