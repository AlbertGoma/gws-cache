use core::{
	ptr::NonNull,
	hash::{
		BuildHasher,
		Hash,
		Hasher
	},
	mem,
	fmt::Debug,
	//marker::PhantomData
};
use ahash::RandomState;
use hashbrown::raw::RawTable;
use std::sync::atomic::{AtomicBool, Ordering};


type Link<K, V> = Option<NonNull<Node<K, V>>>;
pub type DefaultHashBuilder = RandomState;

#[derive(Debug)]
pub struct Node<K, V> {
	k: K,
	v: V,
	p: Link<K, V>,
	n: Link<K, V>,
}

impl<K, V> Node<K, V> {
	fn new(k: K, v: V) -> Self {
		Self { k, v, p: None, n: None }
	}
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
	//marker: PhantomData<Box<Node<K, V>>>, //necessary?
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
			//marker: PhantomData,
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
			//marker: PhantomData,
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
	K: Eq + Hash + Debug, 
	V: Debug,
	S: BuildHasher,
{

	/// Inserts a key-value pair into the cache at the head of the list 
	/// and returns the old value if there is one.
	pub async fn push_front(&mut self, k: K, v: V) -> Option<V> {
		let hb = &self.hash_builder;
		let hash = make_hash(hb, &k);
		let ret: Option<V>;
		
		//Lock:
		while self.lock.compare_and_swap(false, true, Ordering::Acquire) {}
			unsafe {
				let ptr: *mut Node<K, V>;

				//TODO: increment self.bytes

				//Find in HashMap and replace value or insert new node:
				if let Some(item) = self.table.find(hash, |x| k.eq(&x.k)) {
					ret = Some(mem::replace(&mut item.as_mut().v, v));
					ptr = item.as_ptr();
				} else {
					ptr = self.table.insert(hash, Node::new(k, v), |x| make_hash(hb, &x.k)).as_ptr();
					ret = None;
				}
				
				//Move node to head:
				if let Some(mut head) = self.head {	//-> One or more entries in cache
					if head.as_ptr() != ptr {					//-> Node not alredy in head
						(&mut *ptr).n = self.head;
						head.as_mut().p = NonNull::new(ptr);
						self.head = NonNull::new(ptr);
					}
				} else {														//-> Empty cache
					self.head = NonNull::new(ptr);
					self.tail = NonNull::new(ptr);
				}
				
				#[cfg(debug_assertions)]
				println!("Head=({:?})\t{:?}\nTail=({:?})\t{:?}\n",
					self.head.unwrap().as_ptr(),
					self.head.unwrap().as_ref(),
					self.tail.unwrap().as_ptr(),
					self.tail.unwrap().as_ref());
			}

		//Unlock:
		self.lock.store(false, Ordering::Release);
		ret
	}
	
	
	/// Returns the key and value of the Least Frequently Used item in the
	/// cache unless it's empty.
	pub async fn pop_back(&mut self) -> Option<(K, V)> {
		let ret: Option<(K, V)>;
		
		//Lock:
		while self.lock.compare_and_swap(false, true, Ordering::Acquire) {}
			if let Some(ptr) = self.tail {
				unsafe {
					
					#[cfg(debug_assertions)]
					println!("Head=({:?})\t{:?}\nTail=({:?})\t{:?}\n",
						self.head.unwrap().as_ptr(),
						self.head.unwrap().as_ref(),
						self.tail.unwrap().as_ptr(),
						self.tail.unwrap().as_ref());
			
					//Rescue from hashmap:
					let hash = make_hash(&self.hash_builder, &ptr.as_ref().k);
					if let Some(item) = self.table.find(hash, |x| ptr.as_ref().k.eq(&x.k)) {
						self.table.erase_no_drop(&item);
	
						//Set new tail:
						self.tail = ptr.as_ref().p;
						match self.tail {
							None => self.head = None,
							Some(t) => (*t.as_ptr()).n = None,
						}
						let node = item.read();
						ret = Some((node.k, node.v));
						//TODO: drop node?
					} else {
						ret = None;
					}
				}
			} else {
				ret = None;
			}
		
		//Unlock:
		self.lock.store(false, Ordering::Release);
		ret
	}
	
	//TODO: move_front
	
	//TODO: pub get
}


pub(crate) fn make_hash<K: Hash + ?Sized>(hash_builder: &impl BuildHasher, val: &K) -> u64 {
	let mut state = hash_builder.build_hasher();
	val.hash(&mut state);
	state.finish()
}

//TODO: impl Drop for GWSCache?

//TODO: impl Sync for GWSCache


