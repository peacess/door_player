use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug,Clone,Default)]
pub struct MutexQueue<T>{
    deque: Arc<parking_lot::Mutex<VecDeque<T>>>
}

impl<T: Clone> MutexQueue<T> {
    pub fn pop_front(&self) -> Option<T> {
        let mut lock = self.deque.lock();
        lock.pop_front()
    }
    pub fn pop_back(&self) -> Option<T> {
        let mut lock = self.deque.lock();
        lock.pop_back()
    }

    pub fn push_front(&self, t: T) {
        let mut lock = self.deque.lock();
        lock.push_front(t);
    }

    pub fn push_back(&self, t: T) {
        let mut lock = self.deque.lock();
        lock.push_back(t);
    }

    pub fn back(&self) -> Option<T> {
        let lock = self.deque.lock();
        lock.back().map(|t| t.clone())
    }

    pub fn front(&self) -> Option<T> {
        let lock = self.deque.lock();
        lock.front().map(|t| t.clone())
    }

    pub fn len(&self) -> usize {
        let mut lock = self.deque.lock();
        lock.len()
    }

    pub fn is_empty(&self) -> bool {
        let mut lock = self.deque.lock();
        lock.is_empty()
    }
}