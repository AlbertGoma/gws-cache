use core::{ptr::{self, NonNull}, hash::{BuildHasher, Hash, Hasher}, mem, fmt::Debug/*,marker::PhantomData*/};
use ahash::RandomState;
use hashbrown::raw::RawTable;
use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, borrow::Borrow};
use tokio::io::{AsyncRead, AsyncReadExt};


//FIXME: use raw pointers to avoid multiple memory accesses.
type Link<K, V, M> = Option<NonNull<Node<K, V, M>>>;
pub type DefaultHashBuilder = RandomState;

pub trait Meta {
  fn new() -> Self;
  //fn update(&mut self);
}

pub enum Status<V> {
  Ok(V),                //->200
  NoContent,            //->204
  Partial(V),           //->206
  NotModified,          //->304
  //BadRequest,           //->400
  Forbidden,            //->403
  NotFound,             //->404
  RangeNotSatisfiable,  //->416
  Error,                //->500 Don't Panic!
}

//TODO: V: ?Sized ????
#[derive(Debug)]
pub struct Node<K, V, M: Meta> {
  kv: Arc<(K, V)>,
  m: M, //->Metadata
  p: Link<K, V, M>,
  n: Link<K, V, M>,
}



/*impl<K, V> Node<K, V, Empty> {
  #[inline]
  fn new(k: K, v: V) -> Self {
    Self::with_meta(k, v, Empty())
  }
}*/


impl<K, V, M: Meta> Node<K, V, M> {
  fn new(k: K, v: V, m: M) -> Self {
    Self { kv: Arc::new((k, v)), m, p: None, n: None }
  }
}

/// An asynchronous LRU cache for [gws](https://github.com/AlbertGoma/gws)
/// using [hashbrown](https://github.com/rust-lang/hashbrown).
pub struct GWSCache<K, V, M: Meta, S = DefaultHashBuilder> { //FIXME: where K, V, Send/Sync??
  pub(crate) hash_builder: S,
  pub(crate) table: RawTable<Node<K, V, M>>,
  pub(crate) head: Link<K, V, M>,
  pub(crate) tail: Link<K, V, M>,
  pub(crate) lock: AtomicBool,
  pub(crate) bytes: usize,
  //marker: PhantomData<Box<Node<K, V, M>>>, //necessary?
}



impl<K, V, M: Meta> GWSCache<K, V, M, DefaultHashBuilder> {

  // All the capacity must be preallocated: Otherwise the resize() function would slow down
  // accesses by copying all the nodes to a new table and fucking up all our pointers'
  // consistency during the process!!!
  #[inline]
  pub fn new(capacity: usize) -> Self {
    Self::with_hasher(capacity, DefaultHashBuilder::default())
  }
}



impl<K, V, M: Meta, S> GWSCache<K, V, M, S> {
  pub fn with_hasher(capacity: usize, hash_builder: S) -> Self {
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



impl<K, V, M, S> GWSCache<K, V, M, S>
where
  K: Eq + Hash + Debug, 
  V: Debug,
  M: Debug + Meta,
  S: BuildHasher,
{

  /// Inserts a key-value pair into the cache at the head of the list 
  /// and returns the old value if there is one.
  pub async fn push_front(&mut self, k: K, v: V) -> Option<Arc<(K, V)>> {
    let ret: Option<Arc<(K, V)>>;
    
    self.lock();
    let hash = make_hash(&self.hash_builder, &k);
    
      unsafe {
        let ptr: *mut Node<K, V, M>; //FIXME: adapt to metadata

        //TODO: increment self.bytes
        //FIXME: control RawTable capacity!!!!

        //Find in HashMap and replace value or insert new node:
        if let Some(item) = self.table.find(hash, |x| k.eq(&x.kv.0)) {
          //ret = Some(mem::replace(&mut item.as_mut().v, v));
          ret = Some(mem::replace(&mut item.as_mut().kv, Arc::new((k, v))));
          ptr = item.as_ptr();
        } else {
          //FIXME:set initial metadata when there is:
          ptr = self.table.insert_no_grow(hash, Node::new(k, v, Meta::new())).as_ptr();
          ret = None;
        }
        
        //Move node to head:
        if let Some(mut head) = self.head {  //-> One or more entries in cache
          if head.as_ptr() != ptr {          //-> Node not alredy in head
            (&mut *ptr).n = self.head;
            head.as_mut().p = NonNull::new(ptr);
            self.head = NonNull::new(ptr);
          }
        } else {                             //-> Cache is empty
          self.head = NonNull::new(ptr);
          self.tail = NonNull::new(ptr);
        }
        
        #[cfg(debug_assertions)]
        println!("Push Front:\nHead=({:?})\t{:?}\nTail=({:?})\t{:?}\n",
          self.head.unwrap().as_ptr(),
          self.head.unwrap().as_ref(),
          self.tail.unwrap().as_ptr(),
          self.tail.unwrap().as_ref());
      }

    self.unlock();
    ret
  }


  /// Returns the key and value of the Least Frequently Used item in the
  /// cache unless it's empty.
  pub async fn pop_back(&mut self) -> Option<Arc<(K, V)>> {
    let ret: Option<Arc<(K, V)>>;

    self.lock();
      if let Some(ptr) = self.tail {
        unsafe {

          #[cfg(debug_assertions)]
          println!("Pop Back:\nHead=({:?})\t{:?}\nTail=({:?})\t{:?}\n",
            self.head.unwrap().as_ptr(),
            self.head.unwrap().as_ref(),
            self.tail.unwrap().as_ptr(),
            self.tail.unwrap().as_ref());

          //Find in HashMap:
          let hash = make_hash(&self.hash_builder, &ptr.as_ref().kv.0);
          if let Some(item) = self.table.find(hash, |x| ptr.as_ref().kv.0.eq(&x.kv.0)) {
            ret = Some(ptr::read(&item.as_ref().kv as *const Arc<(K, V)>));
            self.table.erase_no_drop(&item);

            //Set new tail:
            self.tail = ptr.as_ref().p;
            match self.tail {
              None => self.head = None,
              Some(t) => (*t.as_ptr()).n = None,
            }
            
          } else {
            ret = None;
          }
        }
      } else {
        ret = None;
      }

    self.unlock();
    ret
  }


  pub async fn get<Q: ?Sized>(&mut self, k: &Q) -> Option<Arc<(K, V)>>
  where
    K: Borrow<Q>,
    Q: Hash + Eq,
  {
    let ret: Option<Arc<(K, V)>>;
    let hash = make_hash(&self.hash_builder, k);
    let mut ptr: Link<K, V, M> = None;
    let mut prv: Link<K, V, M> = None;
    let mut nxt: Link<K, V, M> = None;
    
    self.lock();
      //Retrieve node from HashMap:
      ret = self.table
                .find(hash, |x| k.eq(&x.kv.0.borrow()))
                .map(|item| unsafe {
                  let Node{kv, p, n, ..} = item.as_ref();
                  ptr = NonNull::new(item.as_ptr());
                  nxt = *n;
                  prv = *p;
                  Arc::clone(kv)
                });
                
      //Move node to head:
      match (ptr, prv, nxt) {
        (Some(mut t), Some(mut p), Some(mut n)) => {  //-> Node in the middle
          #[cfg(debug_assertions)]
          println!("Get middle node: {:?} (ptr={:?}, prv={:?}, nxt={:?})\n", &ret, ptr, prv, nxt);
          unsafe {
            p.as_mut().n = Some(n);                   //Set previous' next to self.next
            n.as_mut().p = Some(p);                   //Set next's previous to self.previous
            self.head.unwrap().as_mut().p = Some(t);  //Set old head node's previous to self
            t.as_mut().n = self.head.replace(t);      //Put in head and set self.next to old head node
          }
        },
        (Some(mut t), Some(mut p), None) => {         //-> Node at the tail
          #[cfg(debug_assertions)]
          println!("Get tail node: {:?} (ptr={:?}, prv={:?}, nxt={:?})\n", &ret, ptr, prv, nxt);
          unsafe {
            p.as_mut().n = None;                      //Set new tail node's next to None
            self.tail.replace(p);                     //Set new tail
            self.head.unwrap().as_mut().p = Some(t);  //Set old head's previous to self
            t.as_mut().n = self.head.replace(t);      //Set self at head with next pointing to old head
            t.as_mut().p = None;                      //Set new head's previous to None
          }
        },
        _ => {                                        //-> Node nonexistent or already at head
          #[cfg(debug_assertions)]
          println!("Get head/single or nonexistent node: {:?} (ptr={:?}, prv={:?}, nxt={:?})\n", &ret, ptr, prv, nxt);
         ()
        }
      }
    self.unlock();
    ret
  }

  /*pub async fn get_stream<D, G, P>(k: K, data_source: D, ranges: Option<&[(usize, usize)]>, meta_generator: G, meta_processor: P) -> Status<V>
  where
    D: AsyncRead,
    G: Fn(D) -> M,
    P: Fn(D) -> Status<V>,
  {
    Status::Error
  }*/
  
  /*pub fn get_range() -> Result<V, E> {
  }*/
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


