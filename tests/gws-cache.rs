use gws_cache::{GWSCache, DefaultHashBuilder};
use futures::executor::block_on;


#[test]
fn new_cache() {
	type GWSC = GWSCache<u64, i64>;

	let t = GWSC::new();
	assert_eq!(t.capacity(), 0);
	
	let t = GWSC::with_capacity(0);
	assert_eq!(t.capacity(), 0);
	
	let t = GWSC::with_hasher(DefaultHashBuilder::default());
	assert_eq!(t.capacity(), 0);
	
	let t = GWSC::with_capacity_and_hasher(0, DefaultHashBuilder::default());
	assert_eq!(t.capacity(), 0);
}

#[test]
fn push_pop() {
	type GWSC = GWSCache<u8, &'static str>;
	
	let mut c = GWSC::new();
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

//TODO: test concurrent!
