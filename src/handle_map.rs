use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// An Handle is a typed index into a table of some sort of data
pub trait Handle: Copy + Eq {
    fn new(usize) -> Self;
    fn index(self) -> usize;
}

pub struct HandleMap<K, V> where K: Handle {
    keys: PhantomData<K>,
    values: Vec<V>,
}

impl<K, V> Default for HandleMap<K, V> where K: Handle {
    fn default() -> Self {
        HandleMap {
            keys: PhantomData,
            values: Vec::default(),
        }
    }
}

impl<K, V> HandleMap<K, V> where K: Handle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    fn contains_key(&self, k: K) -> bool {
        k.index() < self.values.len()
    }

    pub fn get(&self, k: K) -> Option<&V> {
        self.values.get(k.index())
    }

    pub fn keys(&self) -> Keys<K> {
        Keys {
            pos: 0,
            len: self.values.len(),
            key: PhantomData,
        }
    }

    pub fn push(&mut self, v: V) -> K {
        let k = self.next_key();
        self.values.push(v);
        k
    }

    fn next_key(&self) -> K {
        K::new(self.values.len())
    }

    pub fn swap(&mut self, a: K, b: K) {
        self.values.swap(a.index(), b.index())
    }
}

impl<K, V> HandleMap<K, V> where K: Handle, V: Clone + Default {
    pub fn with_capacity_default(n: usize, default: V) -> Self {
        let map = HandleMap {
            keys: PhantomData,
            values: vec![default; n],
        };
        map
    }

    pub fn with_capacity(n: usize) -> Self {
        Self::with_capacity_default(n, V::default())
    }

    pub fn resize(&mut self, n: usize) {
        self.values.resize(n, V::default());
    }

    pub fn ensure(&mut self, k: K) -> &mut V {
        if !self.contains_key(k) {
            self.resize(k.index() + 1);
        }
        &mut self.values[k.index()]
    }
}

impl<K, V> Index<K> for HandleMap<K, V> where K: Handle {
    type Output = V;

    fn index(&self, k: K) -> &V {
        &self.values[k.index()]
    }
}

impl<K, V> IndexMut<K> for HandleMap<K, V> where K: Handle {
    fn index_mut(&mut self, k: K) -> &mut V {
        &mut self.values[k.index()]
    }
}

pub struct Keys<K> {
    pos: usize,
    len: usize,
    key: PhantomData<K>,
}

impl<K> Iterator for Keys<K> where K: Handle {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.len {
            let key = K::new(self.pos);
            self.pos += 1;
            Some(key)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len - self.pos;
        (len, Some(len))
    }
}

impl<K> DoubleEndedIterator for Keys<K> where K: Handle {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.pos < self.len {
            self.len -= 1;
            let key = K::new(self.len);
            Some(key)
        } else {
            None
        }
    }
}

impl<K> ExactSizeIterator for Keys<K> where K: Handle {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    struct Ref(u32);

    impl Handle for Ref {
        fn new(index: usize) -> Self { Ref(index as u32) }
        fn index(self) -> usize { self.0 as usize }
    }

    #[test]
    fn map() {
        let mut map: HandleMap<Ref, _> = HandleMap::new();
        let k1 = map.push(12);
        let k2 = map.push(34);

        assert_eq!(map[k1], 12);
        assert_eq!(map[k2], 34);
    }
}
