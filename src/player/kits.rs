use std::sync::Arc;

use ffmpeg::{Rational, Rescale};
use ringbuf::HeapRb;

use crate::player::MILLISEC_TIME_BASE;

pub type RingBufferProducer<T> = ringbuf::Producer<T, Arc<HeapRb<T>>>;
pub type RingBufferConsumer<T> = ringbuf::Consumer<T, Arc<HeapRb<T>>>;

pub fn is_ffmpeg_eof_error(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ffmpeg::Error>(),
        Some(ffmpeg::Error::Eof)
    )
}

pub fn timestamp_to_millisecond(timestamp: i64, time_base: Rational) -> i64 {
    timestamp.rescale(time_base, MILLISEC_TIME_BASE)
}


#[cfg(test)]
mod test {
    #[test]
    fn test_for() {
        let len = 2;
        for i in 1..len {
            println!("{}", i);
        }
    }
}