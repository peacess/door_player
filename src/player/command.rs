use bytemuck::NoUninit;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum CommandUi {
    None,
    FullscreenToggle,
    FullscreenTrue,
    FullscreenFalse,
    MaximizedToggle,
    MaximizedTrue,
    MaximizedFalse,
    MinimizedToggle,
    MinimizedTrue,
    MinimizedFalse,
    Close,
}

unsafe impl NoUninit for CommandUi {}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum CommandGo {
    None,
    Packet(i32),
    Frame(i32),
    //快进的时间
    GoMs(i64),
    //frame number
    Seek(i64),
}

unsafe impl NoUninit for CommandGo {}
