pub use core::sync::atomic::{fence, Ordering};
use std::sync::Arc;

/// Simple concurrency of primitive values.
#[derive(Clone)]
pub struct Shared<T: bytemuck::NoUninit> {
    raw_value: Arc<atomic::Atomic<T>>,
}

impl<T: bytemuck::NoUninit> Shared<T> {
    pub fn set(&self, value: T) {
        self.raw_value.store(value, Ordering::Relaxed)
    }
    pub fn get(&self) -> T {
        self.raw_value.load(Ordering::Relaxed)
    }
    pub fn new(value: T) -> Self {
        Self {
            raw_value: Arc::new(atomic::Atomic::new(value)),
        }
    }
}
