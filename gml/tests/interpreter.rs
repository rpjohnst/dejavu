use std::collections::HashMap;
use std::convert::TryFrom;
use std::io;
use std::ops::Range;

use gml::{Function, Item, symbol::Symbol, vm};

/// Read script arguments.
#[test]
fn arguments() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let select = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"select", body: b"{
        return argument0 + argument1
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let arguments = vec![vm::Value::from(3), vm::Value::from(5)];
    assert_eq!(thread.execute(&mut world, &mut assets, select, arguments)?, vm::Value::from(8));

    let a = Symbol::intern(b"a");
    let b = Symbol::intern(b"b");
    let ab = Symbol::intern(b"ab");
    let arguments = vec![vm::Value::from(a), vm::Value::from(b)];
    assert_eq!(thread.execute(&mut world, &mut assets, select, arguments)?, vm::Value::from(ab));

    Ok(())
}

/// Read and write member variables.
#[test]
fn member() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let member = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"member", body: b"{
        a = 3
        b[3] = 5
        var c;
        c = self.a + self.b[3]
        return c
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let (_, entity) = world.create_instance();
    thread.set_self(entity);

    assert_eq!(thread.execute(&mut world, &mut assets, member, vec![])?, vm::Value::from(8));
    Ok(())
}

/// Read and write builtin variables.
#[test]
fn builtin() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let mut items = HashMap::new();

    let scalar = Symbol::intern(b"scalar");
    items.insert(scalar, Item::Member(Some(Instance::get_scalar), Some(Instance::set_scalar)));

    let array = Symbol::intern(b"array");
    items.insert(array, Item::Member(Some(Instance::get_array), Some(Instance::set_array)));

    let global_scalar = Symbol::intern(b"global_scalar");
    items.insert(global_scalar, Item::Member(Some(World::get_global_scalar), Some(World::set_global_scalar)));

    let global_array = Symbol::intern(b"global_array");
    items.insert(global_array, Item::Member(Some(World::get_global_array), Some(World::set_global_array)));

    let builtin = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"builtin", body: b"{
        scalar = 3
        array[0] = 2 + scalar
        array[1] = scalar + array[0]
        global_scalar = array[0] + array[1]
        global_array[0] = array[1] + global_scalar
        global_array[1] = global_scalar + global_array[0]
        return global_array[1]
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let (_, entity) = world.create_instance();
    world.instances.insert(entity, Instance::default());
    thread.set_self(entity);

    assert_eq!(thread.execute(&mut world, &mut assets, builtin, vec![])?, vm::Value::from(34));

    let instance = &world.instances[&entity];
    assert_eq!(instance.scalar, 3.0);
    assert_eq!(instance.array[0], 5);
    assert_eq!(instance.array[1], 8);
    assert_eq!(world.global_scalar, 13);
    assert_eq!(world.global_array[0], 21.0);
    assert_eq!(world.global_array[1], 34.0);

    Ok(())
}

/// Read and write global variables.
#[test]
fn global() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let global = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"global", body: b"{
        a = 3
        global.a = 5
        globalvar a;
        return self.a + a
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let (_, entity) = world.create_instance();
    thread.set_self(entity);

    assert_eq!(thread.execute(&mut world, &mut assets, global, vec![])?, vm::Value::from(8));
    Ok(())
}

#[test]
fn with_scopes() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let with = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"with", body: b"{
        c = 3
        with (all) {
            n = other.c
            m = other.c
        }
        with (argument0) {
            n = 5
        }
        with (argument1) {
            m = 13
        }
        return argument0.n + argument1.n + argument0.m + argument1.m
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let (a, entity) = world.create_instance();
    let (b, _) = world.create_instance();
    thread.set_self(entity);

    let arguments = vec![vm::Value::from(a), vm::Value::from(b)];
    assert_eq!(thread.execute(&mut world, &mut assets, with, arguments)?, vm::Value::from(24.0));
    Ok(())
}

#[test]
fn with_iterator() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let mut items = HashMap::new();

    let with = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"with", body: b"{
        with (all) {
            c = 3
            var i;
            i = create_instance()
            i.c = 5
        }
        var s;
        s = 0
        with (all) {
            s += c
        }
        return s
    }" });

    let create_instance = Symbol::intern(b"create_instance");
    items.insert(create_instance, Item::Native(World::native_create_instance, 0, false));

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let (_, entity) = world.create_instance();
    world.create_instance();
    thread.set_self(entity);

    assert_eq!(thread.execute(&mut world, &mut assets, with, vec![])?, vm::Value::from(16.0));
    Ok(())
}

/// Read and write arrays.
#[test]
fn array() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let array = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"array", body: b"{
        var a, b, c;
        a[0] = 3
        a[1] = 5
        b = 8
        b[2] = 13
        c[1, 1] = 21
        return a + a[1] + b[0] + b[1] + b[2] + c + c[1, 1]
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    assert_eq!(thread.execute(&mut world, &mut assets, array, vec![])?, vm::Value::from(50));
    Ok(())
}

/// First write to a local is control-dependent.
///
/// Regression test to ensure conditionally-initialized values don't break the compiler.
#[test]
fn conditional_initialization() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    game.scripts.push(project::Script { name: b"fibonacci", body: b"{
        var t;
        if (true) {
            t = 1
        }
        return t
    }" });

    let _: vm::Assets<World, Assets> = gml::build(&game, &items, io::stderr)
        .unwrap_or_else(|_| panic!());
    Ok(())
}

/// Use of undef caused by dead code not dominated by entry.
///
/// Regression test to ensure uses of undef don't break the register allocator.
#[test]
fn dead_undef() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    game.scripts.push(project::Script { name: b"switch", body: b"{
        var i;
        return 0
        return i
    }" });

    let _: vm::Assets<World, Assets> = gml::build(&game, &items, io::stderr)
        .unwrap_or_else(|_| panic!());
    Ok(())
}

/// For loop working with locals.
#[test]
fn for_loop() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let factorial = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"factorial", body: b"{
        var i, j;
        j = 1
        for (i = 1; i <= 4; i += 1) {
            j *= i
        }
        return j
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    assert_eq!(thread.execute(&mut world, &mut assets, factorial, vec![])?, vm::Value::from(24));
    Ok(())
}

/// Control flow across a switch statement.
#[test]
fn switch() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let switch = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"switch", body: b"{
        var i;
        switch (argument0) {
        case 3:
            return 5
        case 8:
            i = 13
            break
        default:
            return 21
        }
        return i
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let arguments = vec![vm::Value::from(3)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(5));

    let arguments = vec![vm::Value::from(8)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(13));

    let arguments = vec![vm::Value::from(21)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(21));

    let arguments = vec![vm::Value::from(34)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(21));

    Ok(())
}

/// An empty switch statement.
#[test]
fn switch_empty() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    game.scripts.push(project::Script { name: b"switch", body: b"{
        switch (argument0) {
        }
    }" });

    let _: vm::Assets<World, Assets> = gml::build(&game, &items, io::stderr)
        .unwrap_or_else(|_| panic!());
    Ok(())
}

/// A switch statement with fallthrough between cases.
#[test]
fn switch_fallthrough() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let switch = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"switch", body: b"{
        var i;
        i = 0
        switch (argument0) {
        case 1:
            i = 3
        case 2:
        case 3:
            i += 5
        }
        return i
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let arguments = vec![vm::Value::from(0)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(0));

    let arguments = vec![vm::Value::from(1)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(8));

    let arguments = vec![vm::Value::from(2)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(5));

    let arguments = vec![vm::Value::from(3)];
    assert_eq!(thread.execute(&mut world, &mut assets, switch, arguments)?, vm::Value::from(5));

    Ok(())
}

/// Call a GML script.
#[test]
fn call_script() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    game.scripts.push(project::Script { name: b"id", body: b"return argument0" });

    let call = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"call", body: b"return id(3) + 5" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    assert_eq!(thread.execute(&mut world, &mut assets, call, vec![])?, vm::Value::from(8));
    Ok(())
}

/// Recursively call a GML script.
#[test]
fn recurse() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let items = HashMap::default();

    let fibonacci = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"fibonacci", body: b"{
        if (argument0 < 2) {
            return 1
        } else {
            return fibonacci(argument0 - 1) + fibonacci(argument0 - 2)
        }
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    let arguments = vec![vm::Value::from(6)];
    assert_eq!(thread.execute(&mut world, &mut assets, fibonacci, arguments)?, vm::Value::from(13));
    Ok(())
}

/// Call a native function.
#[test]
fn ffi() -> Result<(), vm::Error> {
    let mut game = project::Game::default();
    let mut items = HashMap::new();

    let add = Symbol::intern(b"add");
    items.insert(add, Item::Native(World::native_add, 2, false));

    let caller = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"caller", body: b"{
        var a, b, c;
        return call()
    }" });

    game.scripts.push(project::Script { name: b"call", body: b"{
        return add(3, 5) + 8
    }" });

    let code = gml::build(&game, &items, io::stderr).unwrap_or_else(|_| panic!());
    let mut assets = Assets { code };
    let mut world = World::default();
    let mut thread = vm::Thread::default();

    assert_eq!(thread.execute(&mut world, &mut assets, caller, vec![])?, vm::Value::from(16.0));
    Ok(())
}

struct World {
    world: vm::World,

    next_id: i32,
    instances: HashMap<vm::Entity, Instance>,

    global_scalar: i32,
    global_array: [f32; 2],
}

struct Assets {
    code: vm::Assets<World, Self>,
}

impl vm::Api<'_, Assets> for World {
    fn fields<'r>(&'r mut self, assets: &'r mut Assets) ->
        (&'r mut vm::World, &'r mut vm::Assets<World, Assets>)
    { (&mut self.world, &mut assets.code) }
}

impl Default for World {
    fn default() -> Self {
        World {
            world: vm::World::default(),

            next_id: 100001,
            instances: HashMap::default(),

            global_scalar: i32::default(),
            global_array: <[f32; 2]>::default(),
        }
    }
}

impl World {
    fn native_add(
        &mut self, _: &mut Assets, thread: &mut vm::Thread, arguments: Range<usize>
    ) -> Result<vm::Value, vm::ErrorKind> {
        let arguments = unsafe { thread.arguments(arguments) };
        let value = match (arguments[0].borrow().decode(), arguments[1].borrow().decode()) {
            (vm::Data::Real(a), vm::Data::Real(b)) => vm::Value::from(a + b),
            _ => vm::Value::from(0),
        };

        Ok(value)
    }

    fn native_create_instance(
        &mut self, _: &mut Assets, _thread: &mut vm::Thread, _arguments: Range<usize>
    ) -> Result<vm::Value, vm::ErrorKind> {
        let (id, _) = self.create_instance();
        Ok(vm::Value::from(id))
    }

    fn create_instance(&mut self) -> (i32, vm::Entity) {
        let id = self.next_id;
        self.next_id += 1;

        let entity = self.world.create_entity();
        self.world.add_entity(entity, 0, id);
        (id, entity)
    }

    fn get_global_scalar(&mut self, _: &mut Assets, _: vm::Entity, _: usize) -> vm::Value {
        vm::Value::from(self.global_scalar)
    }
    fn set_global_scalar(&mut self, _: &mut Assets, _: vm::Entity, _: usize, value: vm::ValueRef) {
        self.global_scalar = i32::try_from(value).unwrap_or(0);
    }

    fn get_global_array(&mut self, _: &mut Assets, _: vm::Entity, i: usize) -> vm::Value {
        vm::Value::from(self.global_array[i] as f64)
    }
    fn set_global_array(&mut self, _: &mut Assets, _: vm::Entity, i: usize, value: vm::ValueRef) {
        self.global_array[i] = f64::try_from(value).unwrap_or(0.0) as f32;
    }
}

#[derive(Default)]
struct Instance {
    scalar: f32,
    array: [i32; 2],
}

impl Instance {
    pub fn get_scalar(world: &mut World, _: &mut Assets, entity: vm::Entity, _: usize) -> vm::Value {
        let instance = &world.instances[&entity];
        vm::Value::from(instance.scalar as f64)
    }
    pub fn set_scalar(world: &mut World, _: &mut Assets, entity: vm::Entity, _: usize, value: vm::ValueRef) {
        let instance = world.instances.get_mut(&entity).unwrap();
        instance.scalar = f64::try_from(value).unwrap_or(0.0) as f32;
    }

    pub fn get_array(world: &mut World, _: &mut Assets, entity: vm::Entity, i: usize) -> vm::Value {
        let instance = &world.instances[&entity];
        vm::Value::from(instance.array[i])
    }
    pub fn set_array(world: &mut World, _: &mut Assets, entity: vm::Entity, i: usize, value: vm::ValueRef) {
        let instance = world.instances.get_mut(&entity).unwrap();
        instance.array[i] = i32::try_from(value).unwrap_or(0);
    }
}
