use std::collections::{HashMap, HashSet};

use index_map::IndexMap;
use symbol::Symbol;
use vm;

pub struct World {
    next_entity: Entity,
    pub(in vm) hash_components: HashMap<Entity, Scope>,

    pub(in vm) globals: HashSet<Symbol>,
    pub(in vm) objects: HashMap<i32, Vec<Entity>>,
    pub(in vm) instances: IndexMap<i32, Entity>,
}

pub const GLOBAL: Entity = Entity(0);

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Entity(pub i32);

pub type Scope = HashMap<Symbol, vm::Value>;

impl World {
    pub(in vm) fn new() -> Self {
        let mut world = World {
            next_entity: GLOBAL,
            hash_components: HashMap::default(),

            globals: HashSet::default(),
            objects: HashMap::default(),
            instances: IndexMap::default(),
        };

        let global = world.create_entity();
        world.hash_components.insert(global, Scope::new());

        world
    }

    pub(in vm) fn create_entity(&mut self) -> Entity {
        let Entity(entity) = self.next_entity;
        self.next_entity = Entity(entity + 1);
        Entity(entity)
    }

    pub(in vm) fn create_instance(&mut self, id: i32, entity: Entity) {
        self.instances.insert(id, entity);
        self.hash_components.insert(entity, Scope::new());
    }
}
