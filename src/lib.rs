use core::ptr::NonNull;
use ahash::RandomState;
use hashbrown::raw::RawTable;


type Link<K, V> = Option<NonNull<Node<K, V>>>;
pub type DefaultHashBuilder = RandomState;


struct Node<K, V>{
	k: K,
	v: V,
	p: Link<K, V>,
	n: Link<K, V>,
}

/// An asynchronous cache for [gws](https://github.com/AlbertGoma/gws)
/// using [hashbrown](https://github.com/rust-lang/hashbrown).
pub struct GWSCache<K, V, S = DefaultHashBuilder> {
	pub(crate) hash_builder: S,
	pub(crate) table: RawTable<Node<K, V>>,
	pub(crate) head: Link<K, V>,
	pub(crate) tail: Link<K, V>,
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
		}
	}
	
	pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
		Self {
			hash_builder,
			table: RawTable::with_capacity(capacity),
			head: None,
			tail: None,
		}
	}
}
