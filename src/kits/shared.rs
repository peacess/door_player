pub use core::sync::atomic::{fence, Ordering};
use std::sync::Arc;

/// Simple concurrecy of primitive values.
#[derive(Clone)]
pub struct Shared<T: bytemuck::NoUninit> {
    raw_value: Arc<atomic::Atomic<T>>,
}

impl<T: bytemuck::NoUninit> Shared<T> {
    /// Set the value.
    pub fn set(&self, value: T) {
        self.raw_value.store(value, Ordering::Relaxed)
    }
    /// Get the value.
    pub fn get(&self) -> T {
        self.raw_value.load(Ordering::Relaxed)
    }
    /// Make a new cache.
    pub fn new(value: T) -> Self {
        Self {
            raw_value: Arc::new(atomic::Atomic::new(value)),
        }
    }
}

// impl<T: bytemuck::NoUninit> Clone for Shared<T> {
//     fn clone(&self) -> Self {
//         Self {
//             raw_value:  atomic::Atomic::new(self.get())
//         }
//     }
// }