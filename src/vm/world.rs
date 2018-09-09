use std::collections::{HashMap, HashSet};

use index_map::IndexMap;
use symbol::Symbol;
use vm;

pub struct World {
    next_entity: u64,
    pub(in vm) hash_table: HashMap<Entity, Hash>,
    pub(in vm) instance_table: HashMap<Entity, Instance>,

    pub(in vm) globals: HashSet<Symbol>,
    pub(in vm) objects: HashMap<i32, Vec<Entity>>,
    pub(in vm) instances: IndexMap<i32, Entity>,
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Entity(u64);

pub const GLOBAL: Entity = Entity(0);

pub type Hash = HashMap<Symbol, vm::Value>;

#[derive(Default)]
pub struct Instance {
    pub x: f32,
    pub y: f32,
}

impl Instance {
    pub fn get_x(&self) -> vm::Value {
        vm::Value::from(self.x as f64)
    }
    pub fn set_x(&mut self, value: vm::Value) {
        self.x = match value.data() {
            vm::Data::Real(a) => a as f32,
            _ => 0.0,
        };
    }

    pub fn get_y(&self) -> vm::Value {
        vm::Value::from(self.y as f64)
    }
    pub fn set_y(&mut self, value: vm::Value) {
        self.y = match value.data() {
            vm::Data::Real(a) => a as f32,
            _ => 0.0,
        };
    }
}

impl World {
    pub(in vm) fn new() -> Self {
        let Entity(global) = GLOBAL;

        let mut world = World {
            next_entity: global,
            hash_table: HashMap::default(),
            instance_table: HashMap::default(),

            globals: HashSet::default(),
            objects: HashMap::default(),
            instances: IndexMap::default(),
        };

        let global = world.create_entity();
        world.hash_table.insert(global, Hash::new());

        world
    }

    pub(in vm) fn create_entity(&mut self) -> Entity {
        let entity = self.next_entity;
        self.next_entity += 1;
        Entity(entity)
    }

    pub(in vm) fn create_instance(&mut self, id: i32, entity: Entity) {
        self.instances.insert(id, entity);
        self.hash_table.insert(entity, Hash::new());
        self.instance_table.insert(entity, Instance::default());
    }
}
