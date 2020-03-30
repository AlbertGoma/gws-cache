use gws_cache::{GWSCache, DefaultHashBuilder};
use futures::executor::block_on;


#[test]
fn new_cache() {
  type GWSC = GWSCache<u64, i64, i64>;

  let t = GWSC::new(0);
  assert_eq!(t.capacity(), 0);
  
  let t = GWSC::with_hasher(0, DefaultHashBuilder::default());
  assert_eq!(t.capacity(), 0);
}

#[test]
fn push_pop() {
  type GWSC = GWSCache<u8, &'static str, u8>;
  
  let mut c = GWSC::new(5);
  block_on(c.push_front(1, "This"));
  block_on(c.push_front(2, "is"));
  assert_eq!(c.len(), 2);
  block_on(c.push_front(3, "a"));
  block_on(c.push_front(4, "function"));
  block_on(c.push_front(4, "test"));
  assert_eq!(c.len(), 4);

  assert_eq!(block_on(c.pop_back()), Some((1, "This")));
  assert_eq!(block_on(c.pop_back()), Some((2, "is")));
  assert_eq!(block_on(c.pop_back()), Some((3, "a")));
  assert_eq!(block_on(c.pop_back()), Some((4, "test")));
  assert_eq!(block_on(c.pop_back()), None);
}

#[test]
fn get() {
  type KV = GWSCache<&'static str, &'static str, u8>;
  
  let mut c = KV::new(5);
  block_on(c.push_front("a", "first"));
  block_on(c.push_front("b", "second"));
  
  //Getting tail
  assert_eq!(block_on(c.get("a")), Some(("a", &"first")));
  assert_eq!(block_on(c.pop_back()), Some(("b", "second")));
  
  
  block_on(c.push_front("c", "third"));
  block_on(c.push_front("d", "fourth"));
  
  //Getting middle
  assert_eq!(block_on(c.get("b")), None); //miss
  assert_eq!(block_on(c.get("c")), Some(("c", &"third"))); //hit
  assert_eq!(block_on(c.pop_back()), Some(("a", "first")));
  assert_eq!(block_on(c.pop_back()), Some(("d", "fourth")));
  
  //Getting head :)
  block_on(c.push_front("e", "fifth"));
  assert_eq!(block_on(c.get("e")), Some(("e", &"fifth")));
  assert_eq!(block_on(c.pop_back()), Some(("c", "third")));
  assert_eq!(block_on(c.pop_back()), Some(("e", "fifth")));
}

//TODO: test concurrent!

//TODO: test metadata
