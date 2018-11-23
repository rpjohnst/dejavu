#![feature(try_from)]

extern crate gml;

use std::collections::HashMap;
use std::convert::TryFrom;

use gml::{build, Item, symbol::Symbol, vm};

/// Read script arguments.
#[test]
fn arguments() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let select = Symbol::intern("select");
    items.insert(select, Item::Script("{
        return argument0 + argument1
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let arguments = [vm::Value::from(3), vm::Value::from(5)];
    assert_eq!(thread.execute(&mut engine, &resources, select, &arguments)?, vm::Value::from(8));

    let a = Symbol::intern("a");
    let b = Symbol::intern("b");
    let ab = Symbol::intern("ab");
    let arguments = [vm::Value::from(a), vm::Value::from(b)];
    assert_eq!(thread.execute(&mut engine, &resources, select, &arguments)?, vm::Value::from(ab));

    Ok(())
}

/// Read and write member variables.
#[test]
fn member() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let member = Symbol::intern("member");
    items.insert(member, Item::Script("{
        a = 3
        b[3] = 5
        var c;
        c = self.a + self.b[3]
        return c
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let entity = engine.world.create_instance(0, 100001);
    thread.set_self(entity);

    assert_eq!(thread.execute(&mut engine, &resources, member, &[])?, vm::Value::from(8));
    Ok(())
}

/// Read and write builtin variables.
#[test]
fn builtin() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let scalar = Symbol::intern("scalar");
    items.insert(scalar, Item::Member(Some(Instance::get_scalar), Some(Instance::set_scalar)));

    let array = Symbol::intern("array");
    items.insert(array, Item::Member(Some(Instance::get_array), Some(Instance::set_array)));

    let global_scalar = Symbol::intern("global_scalar");
    items.insert(global_scalar, Item::Member(Some(Engine::get_global_scalar), Some(Engine::set_global_scalar)));

    let global_array = Symbol::intern("global_array");
    items.insert(global_array, Item::Member(Some(Engine::get_global_array), Some(Engine::set_global_array)));

    let builtin = Symbol::intern("builtin");
    items.insert(builtin, Item::Script("{
        scalar = 3
        array[0] = 2 + scalar
        array[1] = scalar + array[0]
        global_scalar = array[0] + array[1]
        global_array[0] = array[1] + global_scalar
        global_array[1] = global_scalar + global_array[0]
        return global_array[1]
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let entity = engine.world.create_instance(0, 100001);
    engine.instances.insert(entity, Instance::default());
    thread.set_self(entity);

    assert_eq!(thread.execute(&mut engine, &resources, builtin, &[])?, vm::Value::from(34));

    let instance = &engine.instances[&entity];
    assert_eq!(instance.scalar, 3.0);
    assert_eq!(instance.array[0], 5);
    assert_eq!(instance.array[1], 8);
    assert_eq!(engine.global_scalar, 13);
    assert_eq!(engine.global_array[0], 21.0);
    assert_eq!(engine.global_array[1], 34.0);

    Ok(())
}

/// Read and write global variables.
#[test]
fn global() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let global = Symbol::intern("global");
    items.insert(global, Item::Script("{
        a = 3
        global.a = 5
        globalvar a;
        return self.a + a
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let entity = engine.world.create_instance(0, 100001);
    thread.set_self(entity);

    assert_eq!(thread.execute(&mut engine, &resources, global, &[])?, vm::Value::from(8));
    Ok(())
}

#[test]
fn with() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let with = Symbol::intern("with");
    items.insert(with, Item::Script("{
        var a, b;
        a = 100001
        b = 100002
        c = 3
        with (all) {
            n = other.c
            m = other.c
        }
        with (a) {
            n = 5
        }
        with (b) {
            m = 13
        }
        return a.n + b.n + a.m + b.m
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let a = engine.world.create_instance(0, 100001);
    engine.world.create_instance(0, 100002);
    thread.set_self(a);

    assert_eq!(thread.execute(&mut engine, &resources, with, &[])?, vm::Value::from(24.0));
    Ok(())
}

/// Read and write arrays.
#[test]
fn array() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let array = Symbol::intern("array");
    items.insert(array, Item::Script("{
        var a, b, c;
        a[0] = 3
        a[1] = 5
        b = 8
        b[2] = 13
        c[1, 1] = 21
        return a + a[1] + b[0] + b[1] + b[2] + c + c[1, 1]
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    assert_eq!(thread.execute(&mut engine, &resources, array, &[])?, vm::Value::from(50));
    Ok(())
}

/// First write to a local is control-dependent.
///
/// Regression test to ensure conditionally-initialized values don't break the compiler.
#[test]
fn conditional_initialization() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let fibonacci = Symbol::intern("fibonacci");
    items.insert(fibonacci, Item::Script::<()>("{
        var t;
        if (true) {
            t = 1
        }
        return t
    }"));

    build(items);
    Ok(())
}

/// Use of undef caused by dead code not dominated by entry.
///
/// Regression test to ensure uses of undef don't break the register allocator.
#[test]
fn dead_undef() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let switch = Symbol::intern("switch");
    items.insert(switch, Item::Script::<()>("{
        var i;
        return 0
        return i
    }"));

    build(items);
    Ok(())
}

/// For loop working with locals.
#[test]
fn for_loop() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let factorial = Symbol::intern("factorial");
    items.insert(factorial, Item::Script("{
        var i, j;
        j = 1
        for (i = 1; i <= 4; i += 1) {
            j *= i
        }
        return j
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    assert_eq!(thread.execute(&mut engine, &resources, factorial, &[])?, vm::Value::from(24));
    Ok(())
}

/// Control flow across a switch statement.
#[test]
fn switch() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let switch = Symbol::intern("switch");
    items.insert(switch, Item::Script("{
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
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let arguments = [vm::Value::from(3)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(5));

    let arguments = [vm::Value::from(8)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(13));

    let arguments = [vm::Value::from(21)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(21));

    let arguments = [vm::Value::from(34)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(21));

    Ok(())
}

/// An empty switch statement.
#[test]
fn switch_empty() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let switch = Symbol::intern("switch");
    items.insert(switch, Item::Script::<()>("{
        switch (argument0) {
        }
    }"));

    build(items);
    Ok(())
}

/// A switch statement with fallthrough between cases.
#[test]
fn switch_fallthrough() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let switch = Symbol::intern("switch");
    items.insert(switch, Item::Script("{
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
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let arguments = [vm::Value::from(0)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(0));

    let arguments = [vm::Value::from(1)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(8));

    let arguments = [vm::Value::from(2)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(5));

    let arguments = [vm::Value::from(3)];
    assert_eq!(thread.execute(&mut engine, &resources, switch, &arguments)?, vm::Value::from(5));

    Ok(())
}

/// Call a GML script.
#[test]
fn call_script() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let id = Symbol::intern("id");
    items.insert(id, Item::Script("return argument0"));

    let call = Symbol::intern("call");
    items.insert(call, Item::Script("return id(3) + 5"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    assert_eq!(thread.execute(&mut engine, &resources, call, &[])?, vm::Value::from(8));
    Ok(())
}

/// Recursively call a GML script.
#[test]
fn recurse() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let fibonacci = Symbol::intern("fibonacci");
    items.insert(fibonacci, Item::Script("{
        if (argument0 < 2) {
            return 1
        } else {
            return fibonacci(argument0 - 1) + fibonacci(argument0 - 2)
        }
    }"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    let arguments = [vm::Value::from(6)];
    assert_eq!(thread.execute(&mut engine, &resources, fibonacci, &arguments)?, vm::Value::from(13));
    Ok(())
}

/// Call a native function.
#[test]
fn ffi() -> Result<(), vm::Error> {
    let mut items = HashMap::new();

    let add = Symbol::intern("add");
    items.insert(add, Item::Native(Engine::native_add, 2, false));

    let call = Symbol::intern("call");
    items.insert(call, Item::Script("return add(3, 5) + 8"));

    let resources = build(items);
    let mut engine = Engine::default();
    let mut thread = vm::Thread::new();

    assert_eq!(thread.execute(&mut engine, &resources, call, &[])?, vm::Value::from(16.0));
    Ok(())
}

#[derive(Default)]
struct Engine {
    world: vm::World,

    instances: HashMap<vm::Entity, Instance>,

    global_scalar: i32,
    global_array: [f32; 2],
}

impl vm::world::Api for Engine {
    fn state(&self) -> &vm::World { &self.world }
    fn state_mut(&mut self) -> &mut vm::World { &mut self.world }
}

impl Engine {
    fn native_add(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let value = match (arguments[0].data(), arguments[1].data()) {
            (vm::Data::Real(a), vm::Data::Real(b)) => vm::Value::from(a + b),
            _ => vm::Value::from(0),
        };

        Ok(value)
    }

    fn get_global_scalar(&self, _: vm::Entity, _: usize) -> vm::Value {
        vm::Value::from(self.global_scalar)
    }
    fn set_global_scalar(&mut self, _: vm::Entity, _: usize, value: vm::Value) {
        self.global_scalar = i32::try_from(value).unwrap_or(0);
    }

    fn get_global_array(&self, _: vm::Entity, i: usize) -> vm::Value {
        vm::Value::from(self.global_array[i] as f64)
    }
    fn set_global_array(&mut self, _: vm::Entity, i: usize, value: vm::Value) {
        self.global_array[i] = f64::try_from(value).unwrap_or(0.0) as f32;
    }
}

#[derive(Default)]
struct Instance {
    scalar: f32,
    array: [i32; 2],
}

impl Instance {
    pub fn get_scalar(engine: &Engine, entity: vm::Entity, _: usize) -> vm::Value {
        let instance = &engine.instances[&entity];
        vm::Value::from(instance.scalar as f64)
    }
    pub fn set_scalar(engine: &mut Engine, entity: vm::Entity, _: usize, value: vm::Value) {
        let instance = engine.instances.get_mut(&entity).unwrap();
        instance.scalar = f64::try_from(value).unwrap_or(0.0) as f32;
    }

    pub fn get_array(engine: &Engine, entity: vm::Entity, i: usize) -> vm::Value {
        let instance = &engine.instances[&entity];
        vm::Value::from(instance.array[i])
    }
    pub fn set_array(engine: &mut Engine, entity: vm::Entity, i: usize, value: vm::Value) {
        let instance = engine.instances.get_mut(&entity).unwrap();
        instance.array[i] = i32::try_from(value).unwrap_or(0);
    }
}
