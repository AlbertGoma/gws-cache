use gws_cache::{GWSCache, DefaultHashBuilder};


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

//TODO: test push_front
