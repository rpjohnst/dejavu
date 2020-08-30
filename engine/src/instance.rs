use gml::{self, vm};
use crate::{Context, motion};

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

#[gml::bind]
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

    #[gml::api]
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

    #[gml::api]
    pub fn instance_exists(world: &mut vm::World, obj: i32) -> bool {
        if obj == vm::ALL {
            !world.instances.is_empty()
        } else if obj < 100000 {
            world.objects.get(&obj).map_or(false, |entities| !entities.is_empty())
        } else {
            world.instances.contains_key(obj)
        }
    }

    #[gml::api]
    pub fn instance_number(world: &mut vm::World, obj: i32) -> i32 {
        if obj == vm::ALL {
            world.instances.len() as i32
        } else {
            world.objects.get(&obj).map_or(0, |entities| entities.len()) as i32
        }
    }

    #[gml::api]
    pub fn instance_create(
        cx: &mut Context, thread: &mut vm::Thread,
        x: f32, y: f32, object_index: i32
    ) -> Result<i32, Box<vm::Error>> {
        let Context { world, .. } = cx;
        let id = world.instance.next_id;
        world.instance.next_id += 1;

        let entity = Self::instance_create_id(cx, x, y, object_index, id);

        let Context { assets, .. } = cx;
        let event_type = project::event_type::CREATE;
        let create = gml::Function::Event { event_type, event_kind: 0, object_index };
        if assets.code.code.contains_key(&create) {
            thread.with(entity).execute(cx, create, vec![])?;
        }

        Ok(id)
    }

    pub fn instance_create_id(cx: &mut Context, x: f32, y: f32, object_index: i32, id: i32) ->
        vm::Entity
    {
        let Context { world, assets } = cx;

        let persistent = assets.objects[object_index as usize].persistent;

        let crate::World { world, instance, motion, .. } = world;
        let entity = world.create_entity();
        world.add_entity(entity, object_index, id);
        let inst = Instance { object_index, id, persistent };
        instance.instances.insert(entity, inst);
        let instance = motion::Instance::from_pos(x, y);
        motion.instances.insert(entity, instance);

        entity
    }

    #[gml::api]
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

    #[gml::api]
    pub fn action_create_object(
        cx: &mut Context, thread: &mut vm::Thread,
        entity: vm::Entity, relative: bool, obj: i32, mut x: f32, mut y: f32
    ) -> Result<i32, Box<vm::Error>> {
        let Context { world, .. } = cx;

        if relative {
            x += world.motion.instances[entity].x;
            y += world.motion.instances[entity].y;
        }
        Self::instance_create(cx, thread, x, y, obj)
    }

    #[gml::api]
    pub fn action_kill_object(&mut self, world: &mut vm::World, entity: vm::Entity) {
        self.instance_destroy(world, entity);
    }
}
