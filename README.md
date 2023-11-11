# player
   
    Door player is cross-platform and simple, it is a good project for learning rust egui ffmpeg  

Features:  
1. Play mp4/mkv video file  
2. Embedded subtitle  
3. Fast forward by the Packet(not support rewind)  
4. Fast forward by the Frame(not support rewind)  
5. Next/Pre file  
6. Keyboard  
   * Space/Click-left -> toggle play or pause  
   * Esc -> Close  
   * Double Click/F1 -> toggle Full Screen  
   * → Arrow Left  -> Fast forward packets/frames/milliseconds  
   * ← Arrow Right -> Fast rewind milliseconds  
   * ↑ Arrow Up/+  -> Volume +   
   * ↓ Arrow Down/- -> Volume -  
   * Tab -> Tab Seek
   * Ctrl + Tab -> save current position for "Tab"
7. Other

# build
[ffmpeg](https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building)  

## Ubuntu
sudo apt install libass-dev libavdevice-dev

## window

```shell
   git clone https://github.com/microsoft/vcpkg.git
   cd  vcpkg
   .\bootstrap-vcpkg.bat
   .\vcpkg.exe install ffmpeg
   set FFMPEG_DIR=C:/lang/vcpkg/installed/x64-windows
```

# ffmpeg(just record) 
clone the ffmpeg  
cd ffmpeg  
[//]: # (// --enable-libass --enable-avfilter : add filter subtitles  )
./configure --enable-gpl --enable-static --enable-libass --enable-avfilter --enable-libx264 --enable-pic --enable-ffplay --enable-decoder=pcm*
make -j16 && sudo make install && sudo make uninstall  

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

