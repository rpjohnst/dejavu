use std::collections::VecDeque;
use std::ops::{Index, IndexMut};
use std::num::Wrapping;
use std::u32;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Entity(pub(in vm) u32);

const INDEX_BITS: u32 = 24;
const INDEX_MASK: u32 = (1 << INDEX_BITS) - 1;

const GENERATION_BITS: u32 = 8;
const GENERATION_MASK: u32 = (1 << GENERATION_BITS) - 1;

pub struct EntityAllocator {
    generations: Vec<Wrapping<u8>>,
    free: VecDeque<u32>,
}

const MIN_FREE_SLOTS: usize = 1024;

// TODO: SoA in a single allocation?
#[derive(Default)]
pub struct EntityMap<T> {
    data: Vec<Option<Entry<T>>>,
}

struct Entry<T> {
    generation: Wrapping<u8>,
    value: T,
}

impl Entity {
    #[inline]
    fn new(index: usize, generation: Wrapping<u8>) -> Entity {
        debug_assert!(index < (1 << INDEX_BITS));
        let Wrapping(generation) = generation;
        Entity((index as u32) | (generation as u32) << INDEX_BITS)
    }

    #[inline]
    fn index(self) -> usize {
        let Entity(entity) = self;
        (entity & INDEX_MASK) as usize
    }

    #[inline]
    fn generation(self) -> Wrapping<u8> {
        let Entity(entity) = self;
        Wrapping((entity >> INDEX_BITS & GENERATION_MASK) as u8)
    }
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self {
            generations: Vec::default(),
            free: VecDeque::default(),
        }
    }
}

impl EntityAllocator {
    pub fn create(&mut self) -> Entity {
        let index;
        if self.free.len() > MIN_FREE_SLOTS {
            index = self.free.pop_front().unwrap() as usize;
        } else {
            index = self.generations.len();
            self.generations.push(Wrapping(0));
        }

        Entity::new(index, self.generations[index])
    }

    pub fn destroy(&mut self, entity: Entity) {
        let index = entity.index();
        self.generations[index] += Wrapping(1);
        self.free.push_back(index as u32);
    }

    pub fn exists(&mut self, entity: Entity) -> bool {
        self.generations[entity.index()] == entity.generation()
    }
}

impl<T> EntityMap<T> {
    pub fn insert(&mut self, entity: Entity, value: T) -> Option<(Entity, T)> {
        if entity.index() >= self.data.len() {
            let len = entity.index() + 1;
            let additional = len - self.data.len();
            self.data.reserve(additional);
            for _ in self.data.len()..len {
                self.data.push(None);
            }
        }

        let entry = &mut self.data[entity.index()];
        let old = entry.take().map(|Entry { generation, value }| {
            (Entity::new(entity.index(), generation), value)
        });

        let generation = entity.generation();
        *entry = Some(Entry { generation, value });

        old
    }

    pub fn contains_key(&self, entity: Entity) -> bool {
        self.get(entity).is_some()
    }

    pub fn get(&self, entity: Entity) -> Option<&T> {
        if entity.index() >= self.data.len() {
            return None;
        }

        let entry = &self.data[entity.index()];
        entry.as_ref().and_then(|&Entry { generation, ref value }| {
            if entity.generation() == generation {
                Some(value)
            } else {
                None
            }
        })
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        if entity.index() >= self.data.len() {
            return None;
        }

        let entry = &mut self.data[entity.index()];
        entry.as_mut().and_then(|&mut Entry { generation, ref mut value }| {
            if entity.generation() == generation {
                Some(value)
            } else {
                None
            }
        })
    }
}

impl<T> Index<Entity> for EntityMap<T> {
    type Output = T;

    fn index(&self, entity: Entity) -> &Self::Output {
        self.get(entity).expect("no entry found for key")
    }
}

impl<T> IndexMut<Entity> for EntityMap<T> {
    fn index_mut(&mut self, entity: Entity) -> &mut Self::Output {
        self.get_mut(entity).expect("no entry found for key")
    }
}
