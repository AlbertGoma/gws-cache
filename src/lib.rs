use core::{ptr::{self, NonNull}, hash::{BuildHasher, Hash, Hasher}, mem, fmt::Debug};
use ahash::RandomState;
use hashbrown::raw::{RawTable, Bucket};
use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, borrow::Borrow};


type Link<K, V> = Option<NonNull<Node<K, V>>>;
pub type DefaultHashBuilder = RandomState;



#[derive(Debug)]
pub struct Node<K, V> {
  kv: Arc<(K, V)>,
  p: Link<K, V>,
  n: Link<K, V>,
}

impl<K, V> Node<K, V> {
  fn new(k: K, v: V) -> Self {
    Self { kv: Arc::new((k, v)), p: None, n: None }
  }
}



/// An asynchronous LRU cache for [gws](https://github.com/AlbertGoma/gws)
/// using [hashbrown](https://github.com/rust-lang/hashbrown).
pub struct GWSCache<K, V, S = DefaultHashBuilder> {
  pub(crate) hash_builder: S,
  pub(crate) table: RawTable<Node<K, V>>,
  pub(crate) head: Link<K, V>,
  pub(crate) tail: Link<K, V>,
  pub(crate) lock: AtomicBool,
}



impl<K, V> GWSCache<K, V, DefaultHashBuilder> {

  // All the capacity must be preallocated: Otherwise the resize() function would slow down
  // accesses by copying all the nodes to a new table and fucking up all our pointers'
  // consistency during the process!!!
  #[inline]
  pub fn new(capacity: usize) -> Self {
    Self::with_hasher(capacity, DefaultHashBuilder::default())
  }
  
  #[cfg(debug_assertions)]
  pub fn assert_head_tail(&self, hp: Link<K, V>, tn: Link<K, V>) {
    unsafe {
      assert_eq!(self.head.unwrap().as_ref().p, hp);
      assert_eq!(self.tail.unwrap().as_ref().n, tn);
    }
  }
}



impl<K, V, S> GWSCache<K, V, S> {
  pub fn with_hasher(capacity: usize, hash_builder: S) -> Self {
    Self {
      hash_builder,
      table: RawTable::with_capacity(capacity),
      head: None,
      tail: None,
      lock: AtomicBool::new(false),
    }
  }


  #[inline]
  pub fn len(&self) -> usize {
    self.table.len()
  }
  
  #[inline]
  pub fn capacity(&self) -> usize {
    self.table.capacity()
  }
  
  
  #[inline]
  fn lock(&mut self) {
    while self.lock.compare_and_swap(false, true, Ordering::Acquire) {}
  }
  
  #[inline]
  fn unlock(&mut self) {
    self.lock.store(false, Ordering::Release);
  }
  
}



impl<K, V, S> GWSCache<K, V, S>
where
  K: Eq + Hash + Debug, 
  V: Debug,
  S: BuildHasher,
{
  //self.lock must be true to be used safely
  #[inline]
  unsafe fn to_head(&mut self, item: Bucket<Node<K, V>>) -> Bucket<Node<K, V>> {
    let tpn = (NonNull::new(item.as_ptr()), item.as_ref().p, item.as_ref().n);
    
    match tpn {
      (Some(mut t), Some(mut p), Some(mut n)) => {    //-> Node in the middle
        p.as_mut().n = Some(n);                   //Set previous' next to self.next
        n.as_mut().p = Some(p);                   //Set next's previous to self.previous
        self.head.unwrap().as_mut().p = Some(t);  //Set old head node's previous to self
        t.as_mut().n = self.head.replace(t);      //Set self at head with next pointing to old head
        t.as_mut().p = None;                      //Set new head's previous to None
      },
      (Some(mut t), Some(mut p), None) => {           //-> Node at tail
        p.as_mut().n = None;                      //Set new tail node's next to None
        self.tail.replace(p);                     //Set new tail
        self.head.unwrap().as_mut().p = Some(t);  //Set old head's previous to self
        t.as_mut().n = self.head.replace(t);      //Set self at head with next pointing to old head
        t.as_mut().p = None;                      //Set new head's previous to None
      },
      (Some(t), None, None) if self.tail == None => { //-> New node, empty cache
        self.head = Some(t);                      //Set self at head
        self.tail = Some(t);                      //Set self at tail
      },
      (Some(mut t), None, None) => {                  //-> New node, elements in cache
        self.head.unwrap().as_mut().p = Some(t);  //Set old head's previous to self
        t.as_mut().n = self.head.replace(t);      //Set self at head with next pointing to old head
      },
      _ => ()                                         //-> Node already at head
    }
    item
  }

  //self.lock must be true to be used safely
  #[inline]
  unsafe fn remove(&mut self, item: Bucket<Node<K, V>>) -> Option<Bucket<Node<K, V>>> {
    let pn = (item.as_ref().p, item.as_ref().n);
    
    match pn {
      (Some(mut p), Some(mut n)) => {                 //-> Node in the middle
        p.as_mut().n = Some(n);                   //Set previous' next to next
        n.as_mut().p = Some(p);                   //Set next's previous to previous
      },
      (Some(mut p), None) => {                        //-> Node at tail
        self.tail = Some(p);                      //Set tail to previous
        p.as_mut().n = None;                      //Set previous' next to None
      },
      (None, Some(mut n)) => {                        //-> Node at head
        self.head = Some(n);                      //Set head to next
        n.as_mut().p = None;                      //Set next's previous to None
      },
      _ => {                                          //-> Last node in cache (both at head and tail)
        self.tail = None;
        self.head = None;
      }
    }
    
    self.table.erase_no_drop(&item);
    None
  }
  
  //self.lock must be true and capacity must be handled to be used safely
  #[inline]
  unsafe fn upsert(&mut self, k: K, v: V) -> (Bucket<Node<K, V>>, Option<Arc<(K, V)>>) {
    let h = self.h(&k);
    match self.find(&k, h) {
      Some(i) => {
        let kv = mem::replace(&mut i.as_mut().kv, Arc::new((k, v)));
        (i, Some(kv))
      },
      None => (self.table.insert_no_grow(h, Node::new(k, v)), None),
    }
  }
  
  //self.lock must be true to be used safely, otherwise it could return a bucket about to be erased.
  #[inline]
  unsafe fn find<Q: ?Sized>(&self, k: &Q, h: u64) -> Option<Bucket<Node<K, V>>>
  where
    K: Borrow<Q>,
    Q: Eq,
  {
    self.table.find(h, |x| k.eq(&x.kv.0.borrow()))
  }

  #[inline]
  fn h<Q: Hash + ?Sized>(&self, k: &Q) -> u64 {
    let mut state = self.hash_builder.build_hasher();
    k.hash(&mut state);
    state.finish()
  }




  /// Inserts a key-value pair into the cache at the head of the list 
  /// and returns the old value if there is one. Removes the Least 
  /// Frequently Used element if there is not enough space left.
  pub async fn push_front(&mut self, k: K, v: V) -> Option<Arc<(K, V)>> {
    let ret: (Bucket<Node<K, V>>, Option<Arc<(K, V)>>);
    
    self.lock();
      if self.len() >= self.capacity() {
        self.tail
            .and_then(|p| unsafe {
              self.find(&p.as_ref().kv.0, self.h(&p.as_ref().kv.0))
            }).and_then(|i| unsafe {
              self.remove(i)
            });
      }
      unsafe {
        ret = self.upsert(k, v);
        self.to_head(ret.0);
      }
    self.unlock();
    ret.1
  }


  /// Returns the key and value of the Least Frequently Used item in the
  /// cache unless it's empty.
  pub async fn pop_back(&mut self) -> Option<Arc<(K, V)>> {
    let ret: Option<Arc<(K, V)>>;

    self.lock();      
      ret = self.tail
                .and_then(|p| unsafe {
                  self.find(&p.as_ref().kv.0, self.h(&p.as_ref().kv.0))
                }).map(|i| unsafe {
                  let ret = ptr::read(&i.as_ref().kv as *const Arc<(K, V)>);
                  self.remove(i);
                  ret
                });
    self.unlock();
    ret
  }

  /// Returns the key and value of the element that corresponds to the
  /// provided key when it exists. This key-value pair then gets placed
  /// at the head of the cache's list.
  pub async fn get<Q: ?Sized>(&mut self, k: &Q) -> Option<Arc<(K, V)>>
  where
    K: Borrow<Q>,
    Q: Hash + Eq,
  {
    let ret: Option<Arc<(K, V)>>;

    self.lock();
      unsafe {
        ret = self.find(k, self.h(&k))
                  .map(|i| {
                    let Node{kv, ..} = self.to_head(i).as_ref();
                    Arc::clone(kv)
                  });
      }
    self.unlock();
    ret
  }
}

