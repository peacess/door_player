use std::{collections::VecDeque, sync::Arc};

pub type Deque<T> = Arc<parking_lot::Mutex<VecDeque<T>>>;

pub fn new_deque<T>() -> Deque<T> {
    Arc::new(parking_lot::Mutex::new(VecDeque::new()))
}
