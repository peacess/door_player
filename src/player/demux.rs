use std::path;
use ffmpeg::{media};

pub struct Demux{

}

impl Demux {
    pub fn open(
        filename: &str,
    ) -> Result<(
        ffmpeg::format::context::Input,
        ffmpeg::codec::context::Context,
        ffmpeg::codec::context::Context,
    ), anyhow::Error> {
        let filename = path::Path::new(filename);
        // 获取输入流的上下文
        let ifmt_ctx = ffmpeg::format::input(&filename)?;

        // 获取视频解码器
        let video_stream = ifmt_ctx.streams().best(media::Type::Video).ok_or(ffmpeg::Error::InvalidData)?;
        let video_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())?;

        // 获取音频解码器
        let audio_stream = ifmt_ctx.streams().best(media::Type::Audio).ok_or(ffmpeg::Error::InvalidData)?;
        let audio_context = ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())?;

        Ok((ifmt_ctx, video_context, audio_context))
    }
}
