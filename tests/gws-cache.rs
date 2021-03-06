use gws_cache::{GWSCache, DefaultHashBuilder};
use futures::executor::block_on;


#[test]
fn new_cache() {
  type GWSC = GWSCache<u64, i64>;

  let t = GWSC::new(0);
  assert_eq!(t.capacity(), 0);
  
  let t = GWSC::with_hasher(0, DefaultHashBuilder::default());
  assert_eq!(t.capacity(), 0);
}

#[test]
fn push_pop() {
  type GWSC = GWSCache<u8, &'static str>;
  
  let mut c = GWSC::new(5);
  block_on(c.push_front(1, "This"));
  block_on(c.push_front(2, "is"));
  assert_eq!(c.len(), 2);
  block_on(c.push_front(3, "a"));
  block_on(c.push_front(4, "function"));
  
  //Replacing tail
  block_on(c.push_front(1, "this"));
  
  //Replacing middle
  block_on(c.push_front(3, "a real"));
  block_on(c.push_front(4, "lkhsadgbflkhaf"));
  
  //Replacing head
  block_on(c.push_front(4, "test?"));
  
  //Checking head's previous and tail's next
  c.assert_head_tail(None, None);
  
  assert_eq!(c.len(), 4);

  assert_eq!(*block_on(c.pop_back()).unwrap(), (2, "is"));
  assert_eq!(*block_on(c.pop_back()).unwrap(), (1, "this"));
  assert_eq!(*block_on(c.pop_back()).unwrap(), (3, "a real"));
  assert_eq!(*block_on(c.pop_back()).unwrap(), (4, "test?"));
  assert_eq!(block_on(c.pop_back()), None);
}

#[test]
fn get() {
  type KV = GWSCache<&'static str, &'static str>;
  
  let mut c = KV::new(5);
  block_on(c.push_front("a", "first"));
  block_on(c.push_front("b", "second"));
  
  //Getting tail
  assert_eq!(*block_on(c.get("a")).unwrap(), ("a", "first"));
  assert_eq!(*block_on(c.pop_back()).unwrap(), ("b", "second"));
  
  
  block_on(c.push_front("c", "third"));
  block_on(c.push_front("d", "fourth"));
  
  //Getting middle
  assert_eq!(block_on(c.get("b")), None); //miss
  assert_eq!(*block_on(c.get("c")).unwrap(), ("c", "third")); //hit
  assert_eq!(*block_on(c.pop_back()).unwrap(), ("a", "first"));
  assert_eq!(*block_on(c.pop_back()).unwrap(), ("d", "fourth"));
  
  //Getting head :)
  block_on(c.push_front("e", "fifth"));
  let e1 = block_on(c.get("e")).unwrap();
  let e2 = block_on(c.get("e")).unwrap();
  assert_eq!(*e2, ("e", "fifth"));
  assert_eq!(*e1, ("e", "fifth"));
  assert_eq!(*block_on(c.pop_back()).unwrap(), ("c", "third"));
  assert_eq!(*block_on(c.pop_back()).unwrap(), ("e", "fifth"));
}

#[test]
fn capacity() {
  type KV = GWSCache<u8, u8>;
  let mut c = KV::new(5);
  
  for i in 0..20 {
    block_on(c.push_front(i, i % 7));
  }

  for _ in 0..20 {
    println!("{:?}", block_on(c.pop_back()));
  }
}
