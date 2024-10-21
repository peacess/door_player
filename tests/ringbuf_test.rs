use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
#[test]
fn ring_buf_test() {
    let rb = HeapRb::<i32>::new(2);
    let (mut prod, mut cons) = rb.split();
    let th = std::thread::spawn(move || prod.push_slice(&[2, 3]));
    let _ = th.join();
    let t = cons.try_pop();
    if let Some(t) = t {
        println!("{}", t);
    }
}
