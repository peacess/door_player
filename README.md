# player
   
    Door player is cross-platform and simple, it is a good project for learning rust egui ffmpeg  

Features:  
1. Play mp4/mkv video file  
2. Embedded subtitle  
3. Fast Forward by the Packet(not support rewind)  
4. Fast Forward by the Frame(not support rewind)  
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
7. Autoplay next file
8. Decode threads by file size
9. Other  

# build
[ffmpeg](https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building)  

## Ubuntu

```shell
#   ffmpeg 7
   sudo add-apt-repository ppa:ubuntuhandbook1/ffmpeg7 
   sudo apt update  
   sudo apt install libass-dev libavdevice-dev ffmpeg  
   
   note: if you want to downgrade to 6.0.  
   sudo apt install ppa-purge && sudo ppa-purge ppa:ubuntuhandbook1/ffmpeg7
   sudo add-apt-repository ppa:ubuntuhandbook1/ffmpeg6
   sudo apt update  
   sudo apt install libass-dev libavdevice-dev ffmpeg  
   
#   dependencies
   sudo apt install librust-alsa-sys-dev
```

## window

```shell
   git clone https://github.com/microsoft/vcpkg.git
   cd  vcpkg
   .\bootstrap-vcpkg.bat
   .\vcpkg.exe install ffmpeg
   set FFMPEG_DIR=C:/lang/vcpkg/installed/x64-windows
```

# ffmpeg code(just record) 
clone the ffmpeg  
cd ffmpeg  
[//]: # (// --enable-libass --enable-avfilter : add filter subtitles  )  
./configure --enable-gpl --enable-static --enable-libass --enable-avfilter --enable-libx264 --enable-pic --enable-ffplay --enable-decoder=pcm*  
make -j16 && sudo make install && sudo make uninstall  

# font
   the default fonts in egui is not support chinese, so get the free open fonts from web when builds  
get the download url from github url:  
   github url:  https://github.com/wordshub/free-font/blob/master/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E5%BE%AE%E7%B1%B3%E9%BB%91.ttc  
   rule:  https://[github_user_id].github.io/[repo_name]/  , no master branch  
   download url: https://wordshub.github.io/free-font/assets/font/%E4%B8%AD%E6%96%87/%E6%96%87%E6%B3%89%E9%A9%BF%E7%B3%BB%E5%88%97/%E6%96%87%E6%B3%89%E9%A9%BF%E6%AD%A3%E9%BB%91.ttc  
   see: https://github.com/orgs/community/discussions/42655#discussioncomment-5669289  
   https://nvm-sh.github.io/nvm/blob/v0.39.5/install.sh  

# symphonia 
[see](https://github.com/pdeljanov/Symphonia)  
Pure Rust media container and audio decoding library  

# see
[egui-video(player)](https://github.com/n00kii/egui-video)   First version of door player is base on this project   
[small-player](https://github.com/imxood/small-player)   
[ffmpeg-cpal-play-audio](https://github.com/dceddia/ffmpeg-cpal-play-audio/blob/main/src/main.rs#L53)  
[stainless-ffmpeg](https://github.com/nomalab/stainless-ffmpeg/blob/master/examples/play.rs)  
[ffplay源码分析](https://www.cnblogs.com/leisure_chn/p/10301215.html)  
[ffplay源码分析4-音视频同步](https://www.cnblogs.com/leisure_chn/p/10307089.html)  
[FFplay视频同步分析—ffplay.c源码分析](https://ffmpeg.xianwaizhiyin.net/ffplay/video_sync.html)  
[ffplay](https://ffmpeg.org/ffplay.html)  
[ffmpeg](https://ffmpeg.org/)  
[ffmpeg播放器](https://www.cnblogs.com/leisure_chn/p/10047035.html)  
[学习如何使用 FFmpeg 打造自己的播放器](https://cloud.tencent.com/developer/article/1940943)  
[将音视频时钟同步封装成通用模块](https://blog.csdn.net/u013113678/article/details/126898738)  
[FFmpeg 入门(5)：视频同步](https://www.samirchen.com/ffmpeg-tutorial-5/)  
[FFMPEG 硬件解码API介绍](https://zhuanlan.zhihu.com/p/168240163)  
[ffmpeg 时基timebase、时间戳pts/dts、延时控制delay](https://blog.csdn.net/wanggao_1990/article/details/114067251)  
[FFmpeg DTS、PTS和时间戳TIME_BASE详解](https://blog.csdn.net/aiynmimi/article/details/121231246)  


