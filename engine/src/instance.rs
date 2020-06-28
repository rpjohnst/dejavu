use gml::{self, vm};
use crate::motion;

pub struct State {
    pub next_id: i32,
    pub instances: vm::EntityMap<Instance>,
    pub destroyed: Vec<vm::Entity>,
}

pub struct Instance {
    pub object_index: i32,
    pub id: i32,
    pub persistent: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            next_id: 100001,
            instances: vm::EntityMap::default(),
            destroyed: Vec::default(),
        }
    }
}

#[gml::bind(Api)]
impl State {
    #[gml::get(object_index)]
    pub fn get_object_index(&self, entity: vm::Entity) -> i32 {
        self.instances[entity].object_index
    }

    #[gml::get(id)]
    pub fn get_id(&self, entity: vm::Entity) -> i32 { self.instances[entity].id }

    #[gml::get(persistent)]
    pub fn get_persistent(&self, entity: vm::Entity) -> bool {
        self.instances[entity].persistent
    }
    #[gml::set(persistent)]
    pub fn set_persistent(&mut self, entity: vm::Entity, value: bool) {
        self.instances[entity].persistent = value;
    }

    #[gml::get(instance_count)]
    pub fn get_instance_count(world: &vm::World) -> i32 {
        world.instances.len() as i32
    }

    #[gml::get(instance_id)]
    pub fn get_instance_id(&self, world: &vm::World, i: usize) -> i32 {
        let entity = match world.instances.values().get(i) {
            Some(&entity) => entity,
            None => return vm::NOONE,
        };
        self.instances[entity].id
    }

    #[gml::function]
    pub fn instance_find(&mut self, world: &mut vm::World, obj: i32, n: i32) -> i32 {
        let n = n as usize;
        let entity = if obj == vm::ALL {
            match world.instances.values().get(n) {
                Some(&entity) => entity,
                None => return vm::NOONE,
            }
        } else {
            match world.objects.get(&obj) {
                Some(entities) => match entities.get(n) {
                    Some(&entity) => entity,
                    None => return vm::NOONE,
                },
                None => return vm::NOONE,
            }
        };
        self.instances[entity].id
    }

    #[gml::function]
    pub fn instance_exists(world: &mut vm::World, obj: i32) -> bool {
        if obj == vm::ALL {
            !world.instances.is_empty()
        } else if obj < 100000 {
            world.objects.get(&obj).map_or(false, |entities| !entities.is_empty())
        } else {
            world.instances.contains_key(obj)
        }
    }

    #[gml::function]
    pub fn instance_number(world: &vm::World, obj: i32) -> i32 {
        if obj == vm::ALL {
            world.instances.len() as i32
        } else {
            world.objects.get(&obj).map_or(0, |entities| entities.len()) as i32
        }
    }

    #[gml::function]
    pub fn instance_create(
        &mut self, world: &mut vm::World, motion: &mut motion::State,
        x: f32, y: f32, obj: i32
    ) -> Result<i32, vm::ErrorKind> {
        let object_index = obj;

        let id = self.next_id;
        self.next_id += 1;

        let persistent = false;

        let entity = world.create_entity();
        world.add_entity(entity, object_index, id);
        let instance = Instance { object_index, id, persistent };
        self.instances.insert(entity, instance);
        let instance = motion::Instance::from_pos(x, y);
        motion.instances.insert(entity, instance);

        Ok(id)
    }

    #[gml::function]
    pub fn instance_destroy(&mut self, world: &mut vm::World, entity: vm::Entity) {
        let &Instance { object_index, id, .. } = match self.instances.get(entity) {
            Some(instance) => instance,
            None => return,
        };
        world.remove_entity(entity, object_index, id);
        self.destroyed.push(entity);
    }

    pub fn free_destroyed(&mut self, world: &mut vm::World, motion: &mut motion::State) {
        for entity in self.destroyed.drain(..) {
            motion.instances.remove(entity);
            self.instances.remove(entity);
            world.destroy_entity(entity);
        }
    }
}
