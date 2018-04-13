use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Index;
use std::hash::Hash;

pub struct IndexMap<K, V> where K: Eq + Hash {
    keys: HashMap<K, usize>,
    values: Vec<V>,
}

impl<K, V> Default for IndexMap<K, V> where K: Eq + Hash {
    fn default() -> Self {
        IndexMap {
            keys: HashMap::default(),
            values: Vec::default(),
        }
    }
}

impl<K, V> IndexMap<K, V> where K: Eq + Hash {
    pub fn values(&self) -> &[V] {
        &self.values[..]
    }

    pub fn insert(&mut self, key: K, value: V) {
        match self.keys.entry(key) {
            Entry::Occupied(entry) => {
                self.values[*entry.get()] = value;
            }

            Entry::Vacant(entry) => {
                entry.insert(self.values.len());
                self.values.push(value);
            }
        }
    }
}

impl<K, V> Index<K> for IndexMap<K, V> where K: Eq + Hash {
    type Output = V;

    fn index(&self, key: K) -> &V {
        &self.values[self.keys[&key]]
    }
}
