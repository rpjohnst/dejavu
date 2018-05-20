extern crate gml;

use std::path::PathBuf;
use std::collections::HashMap;

use gml::symbol::Symbol;
use gml::front::{self, Lexer, Parser, SourceFile, ErrorHandler};
use gml::back::{self, ssa};
use gml::vm::{self, code};

/// Read script arguments.
#[test]
fn arguments() {
    let mut functions = HashMap::new();

    let select = Symbol::intern("select");
    functions.insert(select, Function::Script("{
        return argument0 + argument1
    }"));

    let resources = build(functions);
    let mut state = vm::State::new();

    let arguments = [vm::Value::from(3), vm::Value::from(5)];
    let result = Ok(vm::Value::from(8));
    assert_eq!(state.execute(&resources, select, &arguments), result);

    let a = Symbol::intern("a");
    let b = Symbol::intern("b");
    let ab = Symbol::intern("ab");
    let arguments = [vm::Value::from(a), vm::Value::from(b)];
    let result = Ok(vm::Value::from(ab));
    assert_eq!(state.execute(&resources, select, &arguments), result);
}

/// Read and write member variables.
#[test]
fn member() {
    let mut functions = HashMap::new();

    let member = Symbol::intern("member");
    functions.insert(member, Function::Script("{
        a = 3
        b[3] = 5
        var c;
        c = self.a + self.b[3]
        return c
    }"));

    let resources = build(functions);
    let mut state = vm::State::new();

    state.create_instance(100001);
    state.set_self(100001);

    let result = Ok(vm::Value::from(8));
    assert_eq!(state.execute(&resources, member, &[]), result);
}

/// Read and write global variables.
#[test]
fn global() {
    let mut functions = HashMap::new();

    let global = Symbol::intern("global");
    functions.insert(global, Function::Script("{
        a = 3
        global.a = 5
        globalvar a;
        return self.a + a
    }"));

    let resources = build(functions);
    let mut state = vm::State::new();

    state.create_instance(100001);
    state.set_self(100001);

    let result = Ok(vm::Value::from(8));
    assert_eq!(state.execute(&resources, global, &[]), result);
}

#[test]
fn with() {
    let mut functions = HashMap::new();

    let with = Symbol::intern("with");
    functions.insert(with, Function::Script("{
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

    let resources = build(functions);
    let mut state = vm::State::new();

    state.create_instance(100001);
    state.create_instance(100002);
    state.set_self(100001);

    let result = Ok(vm::Value::from(24.0));
    assert_eq!(state.execute(&resources, with, &[]), result);
}

/// Read and write arrays.
#[test]
fn array() {
    let mut functions = HashMap::new();

    let array = Symbol::intern("array");
    functions.insert(array, Function::Script("{
        var a, b, c;
        a[0] = 3
        a[1] = 5
        b = 8
        b[2] = 13
        c[1, 1] = 21
        return a + a[1] + b[0] + b[1] + b[2] + c + c[1, 1]
    }"));

    let resources = build(functions);
    let mut state = vm::State::new();

    let result = Ok(vm::Value::from(50));
    assert_eq!(state.execute(&resources, array, &[]), result);
}

/// First write to a local is control-dependent.
///
/// Regression test to ensure conditionally-initialized values don't break the compiler.
#[test]
fn conditional_initialization() {
    let mut functions = HashMap::new();

    let fibonacci = Symbol::intern("fibonacci");
    functions.insert(fibonacci, Function::Script("{
        var t;
        if (true) {
            t = 1
        }
        return t
    }"));

    build(functions);
}

/// Use of undef caused by dead code not dominated by entry.
///
/// Regression test to ensure uses of undef don't break the register allocator.
#[test]
fn dead_undef() {
    let mut functions = HashMap::new();

    let switch = Symbol::intern("switch");
    functions.insert(switch, Function::Script("{
        var i;
        return 0
        return i
    }"));

    build(functions);
}

/// For loop working with locals.
#[test]
fn for_loop() {
    let mut functions = HashMap::new();

    let factorial = Symbol::intern("factorial");
    functions.insert(factorial, Function::Script("{
        var i, j;
        j = 1
        for (i = 1; i <= 4; i += 1) {
            j *= i
        }
        return j
    }"));

    let resources = build(functions);
    let mut state = vm::State::new();

    let result = Ok(vm::Value::from(24));
    assert_eq!(state.execute(&resources, factorial, &[]), result);
}

/// Control flow across a switch statement.
#[test]
fn switch() {
    let mut functions = HashMap::new();

    let switch = Symbol::intern("switch");
    functions.insert(switch, Function::Script("{
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

    let resources = build(functions);
    let mut state = vm::State::new();

    let arguments = [vm::Value::from(3)];
    let result = Ok(vm::Value::from(5));
    assert_eq!(state.execute(&resources, switch, &arguments), result);

    let arguments = [vm::Value::from(8)];
    let result = Ok(vm::Value::from(13));
    assert_eq!(state.execute(&resources, switch, &arguments), result);

    let arguments = [vm::Value::from(21)];
    let result = Ok(vm::Value::from(21));
    assert_eq!(state.execute(&resources, switch, &arguments), result);

    let arguments = [vm::Value::from(34)];
    let result = Ok(vm::Value::from(21));
    assert_eq!(state.execute(&resources, switch, &arguments), result);
}

/// An empty switch statement.
#[test]
fn switch_empty() {
    let mut functions = HashMap::new();

    let switch = Symbol::intern("switch");
    functions.insert(switch, Function::Script("{
        switch (argument0) {
        }
    }"));

    build(functions);
}

/// A switch statement with fallthrough between cases.
#[test]
fn switch_fallthrough() {
    let mut functions = HashMap::new();

    let switch = Symbol::intern("switch");
    functions.insert(switch, Function::Script("{
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

    let resources = build(functions);
    let mut state = vm::State::new();

    let arguments = [vm::Value::from(0)];
    let result = Ok(vm::Value::from(0));
    assert_eq!(state.execute(&resources, switch, &arguments), result);

    let arguments = [vm::Value::from(1)];
    let result = Ok(vm::Value::from(8));
    assert_eq!(state.execute(&resources, switch, &arguments), result);

    let arguments = [vm::Value::from(2)];
    let result = Ok(vm::Value::from(5));
    assert_eq!(state.execute(&resources, switch, &arguments), result);

    let arguments = [vm::Value::from(3)];
    let result = Ok(vm::Value::from(5));
    assert_eq!(state.execute(&resources, switch, &arguments), result);
}

/// Call a GML script.
#[test]
fn call_script() {
    let mut functions = HashMap::new();

    let id = Symbol::intern("id");
    functions.insert(id, Function::Script("return argument0"));

    let call = Symbol::intern("call");
    functions.insert(call, Function::Script("return id(3) + 5"));

    let resources = build(functions);
    let mut state = vm::State::new();

    let result = Ok(vm::Value::from(8));
    assert_eq!(state.execute(&resources, call, &[]), result);
}

/// Recursively call a GML script.
#[test]
fn recurse() {
    let mut functions = HashMap::new();

    let fibonacci = Symbol::intern("fibonacci");
    functions.insert(fibonacci, Function::Script("{
        if (argument0 < 2) {
            return 1
        } else {
            return fibonacci(argument0 - 1) + fibonacci(argument0 - 2)
        }
    }"));

    let resources = build(functions);
    let mut state = vm::State::new();

    let arguments = [vm::Value::from(6)];
    let result = Ok(vm::Value::from(13));
    assert_eq!(state.execute(&resources, fibonacci, &arguments), result);
}

/// Call a native function.
#[test]
fn ffi() {
    let mut functions = HashMap::new();

    let add = Symbol::intern("add");
    functions.insert(add, Function::Native(native_add));
    fn native_add(
        state: &mut vm::State, _resources: &vm::Resources, arguments: vm::Arguments
    ) -> Result<vm::Value, vm::Error> {
        let arguments = state.arguments(arguments);
        let value = match (arguments[0].data(), arguments[1].data()) {
            (vm::Data::Real(a), vm::Data::Real(b)) => vm::Value::from(a + b),
            _ => vm::Value::from(0),
        };

        Ok(value)
    }

    let call = Symbol::intern("call");
    functions.insert(call, Function::Script("return add(3, 5) + 8"));

    let resources = build(functions);
    let mut state = vm::State::new();

    let result = Ok(vm::Value::from(16.0));
    assert_eq!(state.execute(&resources, call, &[]), result);
}

enum Function {
    Script(&'static str),
    Native(vm::NativeFunction),
}

fn build(functions: HashMap<Symbol, Function>) -> vm::Resources {
    let prototypes: HashMap<Symbol, ssa::Opcode> = functions.iter()
        .map(|(&name, resource)| match *resource {
            Function::Script(_) => (name, ssa::Opcode::Call),
            Function::Native(_) => (name, ssa::Opcode::CallNative),
        })
        .collect();

    let mut resources = vm::Resources::default();
    for (name, resource) in functions.into_iter() {
        match resource {
            Function::Script(source) => {
                resources.scripts.insert(name, compile(&prototypes, source));
            }
            Function::Native(function) => {
                resources.functions.insert(name, function);
            }
        }
    }

    resources
}

fn compile(prototypes: &HashMap<Symbol, ssa::Opcode>, source: &str) -> code::Function {
    let source = SourceFile {
        name: PathBuf::from("<test>"),
        source: String::from(source),
    };
    let errors = ErrorHandler;
    let reader = Lexer::new(&source);
    let mut parser = Parser::new(reader, &errors);
    let program = parser.parse_program();
    let codegen = front::Codegen::new(prototypes, &errors);
    let program = codegen.compile(&program);
    let codegen = back::Codegen::new();
    codegen.compile(&program)
}
