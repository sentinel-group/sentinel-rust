use crate::base::ParamKey;
use lru::{KeyRef, LruCache};
use std::borrow::Borrow;
use std::hash::Hash;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, RwLock,
};

pub trait CounterTrait<K = ParamKey>: Send + Sync + std::fmt::Debug + Default + 'static {
    fn with_capacity(cap: usize) -> Self;
    fn cap(&self) -> usize;
    fn add(&self, key: K, value: u64);
    fn add_if_absent(&self, key: K, value: u64) -> Option<Arc<AtomicU64>>;
    #[cfg(not(test))]
    fn get<Q>(&self, key: &Q) -> Option<Arc<AtomicU64>>
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized;
    #[cfg(test)]
    fn get<Q>(&self, key: &Q) -> Option<Arc<AtomicU64>>
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + 'static + Sized;
    #[cfg(not(test))]
    fn remove<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized;
    #[cfg(test)]
    fn remove<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + 'static + Sized;
    #[cfg(not(test))]
    fn contains<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized;
    #[cfg(test)]
    fn contains<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + 'static + Sized;
    fn keys(&self) -> Vec<K>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn purge(&self);
}

#[derive(Debug)]
pub struct Counter<K = ParamKey>
where
    K: Send + Sync + Hash + Eq + std::fmt::Debug + Clone + 'static,
{
    cache: RwLock<LruCache<K, Arc<AtomicU64>>>,
}

/// Counter caches the hotspot parameter
impl<K> CounterTrait<K> for Counter<K>
where
    K: Send + Sync + Hash + Eq + std::fmt::Debug + Clone,
{
    fn with_capacity(cap: usize) -> Counter<K> {
        Counter {
            cache: RwLock::new(LruCache::new(cap)),
        }
    }

    fn cap(&self) -> usize {
        self.cache.read().unwrap().cap()
    }

    /// `add` add a value to the cache,
    /// Updates the "recently used"-ness of the key.
    fn add(&self, key: K, value: u64) {
        let mut cache = self.cache.write().unwrap();
        if cache.contains(&key) {
            cache.get(&key).unwrap().store(value, Ordering::SeqCst);
        } else {
            cache.put(key, Arc::new(AtomicU64::new(value)));
        }
    }

    // If the key is not existed in the cache, adds a value to the cache then return None. And updates the "recently used"-ness of the key
    // If the key is already existed in the cache, do nothing and return the prior value
    fn add_if_absent(&self, key: K, value: u64) -> Option<Arc<AtomicU64>> {
        let mut cache = self.cache.write().unwrap();
        if cache.contains(&key) {
            cache.get(&key).map(Arc::clone)
        } else {
            cache.put(key, Arc::new(AtomicU64::new(value)));
            None
        }
    }

    // `get` returns key's value from the cache and updates the "recently used"-ness of the key.
    fn get<Q>(&self, key: &Q) -> Option<Arc<AtomicU64>>
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.cache.write().unwrap().get(key).map(Arc::clone)
    }

    // `remove` removes a key from the cache.
    // Return true if the key was contained.
    fn remove<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.cache.write().unwrap().pop(key).is_some()
    }

    // `contains` checks if a key exists in cache
    // Without updating the recent-ness.
    fn contains<Q>(&self, key: &Q) -> bool
    where
        KeyRef<K>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.cache.read().unwrap().contains(key)
    }

    // `keys` returns the keys in the cache, from oldest to newest.
    fn keys(&self) -> Vec<K> {
        let cache = self.cache.read().unwrap();
        let keys = cache.iter().rev().map(|(k, _v)| k.clone());
        keys.collect()
    }

    // `len` returns the number of items in the cache.
    fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // `purge` clears all cache entries.
    fn purge(&self) {
        self.cache.write().unwrap().clear()
    }
}

impl<K> Default for Counter<K>
where
    K: Send + Sync + Hash + Eq + std::fmt::Debug + Clone,
{
    fn default() -> Counter<K> {
        Counter::<K>::with_capacity(0)
    }
}

#[cfg(test)]
pub(crate) use test::MockCounter;

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use mockall::predicate::*;
    use mockall::*;

    mock! {
        #[derive(Debug)]
        pub(crate) Counter<K>
        where
        K: Send + Sync +Hash + Eq + std::fmt::Debug + Clone + 'static
        {}
        impl<K> CounterTrait<K> for Counter<K>
        where
        K: Send + Sync +Hash + Eq + std::fmt::Debug + Clone + 'static
        {
            fn with_capacity(cap: usize) -> Self;
            fn cap(&self) -> usize;
            fn add(&self, key: K, value: u64);
            fn add_if_absent(&self, key: K, value: u64) -> Option<Arc<AtomicU64>>;
            fn get<Q>(&self, key: &Q) -> Option<Arc<AtomicU64>>
            where
                KeyRef<K>: Borrow<Q>,
                Q: Hash + Eq + Sized + 'static;
            fn remove<Q>(&self, key: &Q) -> bool
            where
                KeyRef<K>: Borrow<Q>,
                Q: Hash + Eq + Sized + 'static;
            fn contains<Q>(&self, key: &Q) -> bool
            where
                KeyRef<K>: Borrow<Q>,
                Q: Hash + Eq + Sized + 'static;
            fn keys(&self) -> Vec<K>;
            fn len(&self) -> usize;
            fn is_empty(&self) -> bool;
            fn purge(&self);
        }
    }

    #[test]
    fn add_get() {
        let counter = Counter::with_capacity(100);
        for i in 1..=100 {
            counter.add(i.to_string(), i);
        }
        assert_eq!(100, counter.len());
        assert_eq!(
            1,
            counter.get(&"1".to_owned()).unwrap().load(Ordering::SeqCst)
        );
    }

    #[test]
    fn add_if_absent() {
        let counter = Counter::with_capacity(100);
        for i in 1..=99 {
            counter.add(i.to_string(), i);
        }
        let prior = counter.add_if_absent(100.to_string(), 100);
        assert!(prior.is_none());
        let prior = counter.add_if_absent(100.to_string(), 100);
        assert_eq!(100, prior.unwrap().load(Ordering::SeqCst));
        assert_eq!(
            100,
            counter
                .get(&100.to_string())
                .unwrap()
                .load(Ordering::SeqCst)
        );
    }

    #[test]
    fn contains() {
        let counter = Counter::with_capacity(100);
        for i in 1..=100 {
            counter.add(i.to_string(), i);
        }
        assert!(counter.contains(&100.to_string()));
        assert!(counter.contains(&1.to_string()));
        assert!(!counter.contains(&101.to_string()));
        counter.add(101.to_string(), 101);
        assert!(!counter.contains(&1.to_string()));
    }

    #[test]
    fn keys() {
        let counter = Counter::with_capacity(100);
        for i in 1..=100 {
            counter.add(i.to_string(), i);
        }
        assert_eq!(100, counter.keys().len());
        assert_eq!("1", counter.keys()[0]);
        assert_eq!("100", counter.keys()[99]);
    }

    #[test]
    fn purge() {
        let counter = Counter::with_capacity(100);
        for i in 1..=100 {
            counter.add(i.to_string(), i);
        }
        assert_eq!(100, counter.len());
        counter.purge();
        assert_eq!(0, counter.len());
    }

    #[test]
    fn remove() {
        let counter = Counter::with_capacity(100);
        for i in 1..=100 {
            counter.add(i.to_string(), i);
        }
        assert_eq!(100, counter.len());
        counter.remove(&100.to_string());
        assert_eq!(99, counter.len());
        let prior = counter.get(&100.to_string());
        assert!(prior.is_none());
    }
}
