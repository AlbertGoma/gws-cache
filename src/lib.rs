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
