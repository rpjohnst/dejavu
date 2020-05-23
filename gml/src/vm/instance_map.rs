use std::mem;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Index;
use std::hash::Hash;

use crate::rc_vec::RcVec;

/// A hash map that preserves insertion order in a copy-on-write array of values.
///
/// This is designed to map instance IDs to entity handles. Using a copy-on-write `RcVec` for
/// the entity handles allows `with` loops to iterate over them without being invalidated by
/// reallocations or mutations.
pub struct InstanceMap<K, V> where K: Eq + Hash {
    keys: HashMap<K, usize>,
    values: RcVec<V>,
}

impl<K, V> Default for InstanceMap<K, V> where K: Eq + Hash {
    fn default() -> Self {
        InstanceMap {
            keys: HashMap::default(),
            values: RcVec::default(),
        }
    }
}

impl<K, V> InstanceMap<K, V> where K: Eq + Hash {
    pub fn values(&self) -> &RcVec<V> { &self.values }

    pub fn len(&self) -> usize { self.values.len() }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    pub fn contains_key(&self, key: K) -> bool { self.keys.contains_key(&key) }
}

impl<K, V> InstanceMap<K, V> where K: Eq + Hash, V: Clone {
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.keys.entry(key) {
            Entry::Occupied(entry) => {
                let values = self.values.make_mut();
                let old = mem::replace(&mut values[*entry.get()], value);
                Some(old)
            }

            Entry::Vacant(entry) => {
                entry.insert(self.values.len());
                self.values.push(value);
                None
            }
        }
    }

    pub fn remove(&mut self, key: K) -> Option<V> {
        let index = self.keys.remove(&key)?;
        let value = self.values.remove(index);
        for index in self.keys.values_mut().filter(|&&mut i| i > index) {
            *index -= 1;
        }
        Some(value)
    }
}

impl<K, V> Index<K> for InstanceMap<K, V> where K: Eq + Hash {
    type Output = V;

    fn index(&self, key: K) -> &V {
        &self.values[self.keys[&key]]
    }
}
