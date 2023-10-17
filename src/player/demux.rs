use crate::player::play_ctrl::PlayCtrl;

pub struct Demux {
    ctrl: PlayCtrl,
    ifmt_ctx: ffmpeg::format::context::Input,
    // video_queue: Arc<Mutex<PacketQueue>>,
    // audio_queue: Arc<Mutex<PacketQueue>>,
}

impl Demux {
    // pub fn open(
    //     filename: &str,
    //     cmd_rx: Receiver<Command>,
    //     state_tx: Sender<PlayState>,
    //     abort_request: Arc<AtomicBool>,
    // ) -> Result<Self, anyhow::Error> {
    //     let filename = path::Path::new(filename);
    //
    //     let (audio_frame_tx, audio_frame_queue) =  kanal::bounded::<AudioFrame>(AUDIO_FRAME_QUEUE_SIZE);
    //     let (video_frame_tx, video_frame_queue) = kanal::bounded::<VideoFrame>(VIDEO_FRAME_QUEUE_SIZE);
    //
    //
    //     let ctrl = {
    //         let audio_dev = AudioDevice::new()
    //             .map_err(|e| {
    //                 state_tx.send(PlayState::Error(e)).ok();
    //             })
    //             .unwrap();
    //         let audio_dev = Arc::new(RwLock::new(audio_dev));
    //
    //         PlayControl::new(audio_dev,state_tx,abort_request)
    //     };
    //
    // }
}
