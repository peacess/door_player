# player
   This player is cross-platform and not perfect, but it is a good project for learning rust egui ffmpeg  

dependencies: egui, ffmpeg    

# build
[ffmpeg](https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building)  
[sdl2](https://github.com/Rust-SDL2/rust-sdl2)  

## Ubuntu
sudo apt install libass-dev libsdl2-dev libavdevice-dev

# see ffmpeg 
clone the ffmpeg
cd ffmpeg
// --enable-pic : fix the "[swscaler @ 0x7f8da4019600] No accelerated colorspace conversion found from yuv420p to rgb24"
// --enable-libass --enable-avfilter : add filter subtitles

./configure --enable-gpl --enable-static --enable-libass --enable-avfilter --enable-libx264 --enable-pic --enable-ffplay --enable-decoder=pcm*
make -j16 && sudo make install

# see
[egui-video(player)](https://github.com/n00kii/egui-video)   First version of door player is base on this project   
[small-player](https://github.com/imxood/small-player)   
[ffmpeg-cpal-play-audio](https://github.com/dceddia/ffmpeg-cpal-play-audio/blob/main/src/main.rs#L53)  
[stainless-ffmpeg](https://github.com/nomalab/stainless-ffmpeg/blob/master/examples/play.rs)  
[ffplay源码分析](https://www.cnblogs.com/leisure_chn/p/10301215.html)  
[ffplay源码分析4-音视频同步](https://www.cnblogs.com/leisure_chn/p/10307089.html)  
[ffplay](https://ffmpeg.org/ffplay.html)  
[ffmpeg](https://ffmpeg.org/)  
[ffmpeg播放器](https://www.cnblogs.com/leisure_chn/p/10047035.html)  
[学习如何使用 FFmpeg 打造自己的播放器](https://cloud.tencent.com/developer/article/1940943)  
[将音视频时钟同步封装成通用模块](https://blog.csdn.net/u013113678/article/details/126898738)  
[FFmpeg 入门(5)：视频同步](https://www.samirchen.com/ffmpeg-tutorial-5/)  

