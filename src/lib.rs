use core::{ptr::NonNull, hash::{BuildHasher, Hash, Hasher}, mem};
use ahash::RandomState;
use hashbrown::raw::RawTable;
use std::sync::atomic::{AtomicBool, Ordering};


type Link<K, V> = Option<NonNull<Node<K, V>>>;
pub type DefaultHashBuilder = RandomState;


pub struct Node<K, V>{
	k: K,
	v: V,
	p: Link<K, V>,
	n: Link<K, V>,
}

//TODO: impl Drop for Node?

/// An asynchronous LRU cache for [gws](https://github.com/AlbertGoma/gws)
/// using [hashbrown](https://github.com/rust-lang/hashbrown).
pub struct GWSCache<K, V, S = DefaultHashBuilder> { //FIXME: where K, V, Send/Sync??| Arc?
	pub(crate) hash_builder: S,
	pub(crate) table: RawTable<Node<K, V>>,
	pub(crate) head: Link<K, V>,
	pub(crate) tail: Link<K, V>,
	pub(crate) lock: AtomicBool,
	pub(crate) bytes: usize,
}



impl<K, V> GWSCache<K, V, DefaultHashBuilder> {
	pub fn new() -> Self {
		Self::with_hasher(DefaultHashBuilder::default())
	}

	pub fn with_capacity(capacity: usize) -> Self {
		Self::with_capacity_and_hasher(capacity, DefaultHashBuilder::default())
	}
}



impl<K, V, S> GWSCache<K, V, S> {
	pub fn with_hasher(hash_builder: S) -> Self {
		Self {
			hash_builder,
			table: RawTable::new(),
			head: None,
			tail: None,
			lock: AtomicBool::new(false),
			bytes: 0,
		}
	}

	pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
		Self {
			hash_builder,
			table: RawTable::with_capacity(capacity),
			head: None,
			tail: None,
			lock: AtomicBool::new(false),
			bytes: 0,
		}
	}



	pub fn len(&self) -> usize {
		self.table.len()
	}

	pub fn capacity(&self) -> usize {
		self.table.capacity()
	}
}



impl<K, V, S> GWSCache<K, V, S>
where
	K: Eq + Hash,
	S: BuildHasher,
{

	/// Inserts a key-value pair into the cache at the head of the list 
	/// and returns the old value if there is one.
	pub async fn push_front(&mut self, k: K, v: V) -> Option<V> {

		let hash = make_hash(&self.hash_builder, &k);
		let mut node = Node { k, v, p: None, n: None };
		let ptr: *mut Node<K, V> = &mut node;
		let ret: Option<V>;
		
		while self.lock.compare_and_swap(false, true, Ordering::Acquire) {}
			//TODO: increment self.bytes
			
			if let Some(mut head) = self.head {
				node.n = Some(head);
				unsafe {
					head.as_mut().p = NonNull::new(ptr);
				}
				self.head = NonNull::new(ptr);
			} else {
				self.head = NonNull::new(ptr);
				self.tail = NonNull::new(ptr);
			}

			if let Some(item) = self.table.find(hash, |x| node.k.eq(&x.k)) {
				unsafe {
					ret = Some(mem::replace(&mut item.as_mut().v, node.v));
				}
			} else {
				let hash_builder = &self.hash_builder;
				self.table.insert(hash, node, |x| make_hash(hash_builder, &x.k));
				ret = None;
			}
			
		self.lock.store(false, Ordering::Release);
		
		ret
	}
	
	//TODO: pub pop_back
	
	//TODO: move_front
	
	//TODO: pub get
}


pub(crate) fn make_hash<K: Hash + ?Sized>(hash_builder: &impl BuildHasher, val: &K) -> u64 {
	let mut state = hash_builder.build_hasher();
	val.hash(&mut state);
	state.finish()
}

//TODO: impl Drop for GWSCache?


