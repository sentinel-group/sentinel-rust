use lru::{KeyRef, LruCache};
use std::borrow::Borrow;
use std::hash::Hash;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, RwLock,
};

#[derive(Debug)]
pub struct Counter<K: Hash + Eq> {
    cache: RwLock<LruCache<K, Arc<AtomicU64>>>,
}

/// Counter caches the hotspot parameter
impl<K: Hash + Eq> Counter<K> {
    pub fn new(cap: usize) -> Counter<K> {
        Counter {
            cache: RwLock::new(LruCache::new(cap)),
        }
    }

    pub fn cap(&self) -> usize {
        self.cache.read().unwrap().cap()
    }

    /// `add` add a value to the cache,
    /// Updates the "recently used"-ness of the key.
    pub fn add<Q: ?Sized>(&self, key: K, value: u64) {
        let mut cache = self.cache.write().unwrap();
        if cache.contains(&key) {
            cache.get(&key).unwrap().store(value, Ordering::SeqCst);
        } else {
            cache.put(key, Arc::new(AtomicU64::new(value)));
        }
    }

    // If the key is not existed in the cache, adds a value to the cache then return None. And updates the "recently used"-ness of the key
    // If the key is already existed in the cache, do nothing and return the prior value
    pub fn add_if_absent(&self, key: K, value: u64) -> Option<Arc<AtomicU64>> {
        let mut cache = self.cache.write().unwrap();
        if cache.contains(&key) {
            cache.get(&key).map(|v| Arc::clone(v))
        } else {
            cache.put(key, Arc::new(AtomicU64::new(value)));
            None
        }
    }

    // `get` returns key's value from the cache and updates the "recently used"-ness of the key.
    pub fn get<Q>(&self, key: &Q) -> Option<Arc<AtomicU64>>
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.cache.write().unwrap().get(&key).map(|v| Arc::clone(v))
    }

    // `remove` removes a key from the cache.
    // Return true if the key was contained.
    pub fn remove<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.cache.write().unwrap().pop(&key).is_some()
    }

    // `contains` checks if a key exists in cache
    // Without updating the recent-ness.
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.cache.read().unwrap().contains(&key)
    }

    // `keys` returns the keys in the cache, from oldest to newest.
    pub fn keys(&self) -> Vec<&K> {
        self.cache
            .read()
            .unwrap()
            .iter()
            .map(|(k, v)| k)
            .rev()
            .collect()
    }

    // `len` returns the number of items in the cache.
    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    // `purge` clears all cache entries.
    pub fn purge(&self) {
        self.cache.write().unwrap().clear()
    }
}

impl<K: Hash + Eq> Default for Counter<K> {
    fn default() -> Counter<K> {
        Counter::<K>::new(0)
    }
}
