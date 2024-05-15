use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
#[test]
fn ringbuf_test() {
    let mut rb = HeapRb::<i32>::new(2);
    let (mut prod, mut cons) = rb.split();
    let th = std::thread::spawn(move || prod.push_slice(&[2, 3]));
    th.join();
    let t = cons.try_pop();
    if let Some(t) = t {
        println!("{}", t);
    }
}
