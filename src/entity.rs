use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// An Entity is a typed index into a table of some sort of data
pub trait Entity: Copy + Eq {
    fn new(usize) -> Self;
    fn index(self) -> usize;
}

pub struct EntityMap<K, V> {
    keys: PhantomData<K>,
    values: Vec<V>,
}

impl<K, V> EntityMap<K, V> where K: Entity {
    pub fn new() -> Self {
        EntityMap { keys: PhantomData, values: Vec::new() }
    }

    pub fn push(&mut self, v: V) -> K {
        let k = self.next_key();
        self.values.push(v);
        k
    }

    fn next_key(&self) -> K {
        K::new(self.values.len())
    }
}

impl<K, V> Index<K> for EntityMap<K, V> where K: Entity {
    type Output = V;

    fn index(&self, k: K) -> &V {
        &self.values[k.index()]
    }
}

impl<K, V> IndexMut<K> for EntityMap<K, V> where K: Entity {
    fn index_mut(&mut self, k: K) -> &mut V {
        &mut self.values[k.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
    struct Ref(u32);

    impl Entity for Ref {
        fn new(index: usize) -> Self { Ref(index as u32) }
        fn index(self) -> usize { self.0 as usize }
    }

    #[test]
    fn map() {
        let mut map: EntityMap<Ref, _> = EntityMap::new();
        let k1 = map.push(12);
        let k2 = map.push(34);

        assert_eq!(map[k1], 12);
        assert_eq!(map[k2], 34);
    }
}
