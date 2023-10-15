
#[derive(Default, Clone)]
pub struct VideoFrame {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub pts: f64,
    pub duration: f64,
}

impl VideoFrame {
    pub fn new(
        raw_data: *const u8,
        width: usize,
        height: usize,
        line_size: usize,
        pts: f64,
        duration: f64,
    ) -> Self {
        let raw_data = unsafe { std::slice::from_raw_parts(raw_data, height * line_size) };
        let mut data: Vec<u8> = vec![0; width * height * 4];
        let data_slice = data.as_mut_slice();
        for i in 0..height as usize {
            let start = i * width * 4;
            let end = start + width * 4;
            let slice = &mut data_slice[start..end];

            let start = i * line_size;
            let end = start + width * 4;
            slice.copy_from_slice(&raw_data[start..end]);
        }
        Self {
            data,
            width,
            height,
            pts,
            duration,
        }
    }
}