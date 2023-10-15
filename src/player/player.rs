
use crate::player::audio_frame::AudioFrame;
use crate::player::video_frame::VideoFrame;


/// player base ffmpeg, there are 4 threads to player file.
pub struct Player{
    file_path: String,

    audio_frame_receiver: Option<kanal::Receiver<AudioFrame>>,
    video_frame_receiver: Option<kanal::Receiver<VideoFrame>>,
}

impl Player {
    pub fn new(file: &str) -> Player {
        Self{
            file_path: file.to_string(),
            audio_frame_receiver: None,
            video_frame_receiver: None,
        }
    }

    pub fn play(&mut self) {

        let (v_s,v_r) = kanal::bounded(10);
        let (a_s, a_r) = kanal::bounded(10);
        self.audio_frame_receiver = Some(a_r);
        self.video_frame_receiver = Some(v_r);

        //打开文件
        //开启 视频解码线程
        //开启 音频解码线程
        //开启 音频播放线程
        //开启 读frame线程

        //视频播放的计算在 ui线程中，这样计算的时间最准确。 意视频同步是以音频时间为准
    }
}

