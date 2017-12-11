extern crate gml;

use std::path::PathBuf;
use std::collections::HashMap;

use gml::symbol::Symbol;
use gml::front::{self, Lexer, Parser, SourceFile, ErrorHandler};
use gml::back::{self, ssa};
use gml::vm::{self, code};

fn compile(prototypes: &HashMap<Symbol, ssa::Prototype>, source: &str) -> code::Function {
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

#[test]
fn factorial() {
    let factorial = Symbol::intern("factorial");

    let mut prototypes = HashMap::new();
    prototypes.insert(factorial, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    let program = compile(&prototypes, "{
        var i, j;
        j = 1
        for (i = 1; i <= 4; i += 1)
            j *= i
        return j
    }");
    resources.scripts.insert(factorial, program);

    let mut state = vm::State::new();

    let value = vm::Value::from(24);
    assert_eq!(state.execute(&resources, &mut (), factorial, 100001, 100001, &[]), Ok(value));
}

#[test]
fn switch() {
    let switch = Symbol::intern("switch");

    let mut prototypes = HashMap::new();
    prototypes.insert(switch, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    let program = compile(&prototypes, "{
        var i;
        switch (argument0) {
        case 3:
            return 5
        case 8:
            for (i = 0; i < 11; i += 1) {}
            break
        default:
            return 15
        }
        return i
    }");
    resources.scripts.insert(switch, program);

    let mut state = vm::State::new();

    let arguments = &[vm::Value::from(3)];
    let value = vm::Value::from(5);
    assert_eq!(state.execute(&resources, &mut (), switch, 100001, 100001, arguments), Ok(value));

    let arguments = &[vm::Value::from(8)];
    let value = vm::Value::from(11);
    assert_eq!(state.execute(&resources, &mut (), switch, 100001, 100001, arguments), Ok(value));

    let arguments = &[vm::Value::from(13)];
    let value = vm::Value::from(15);
    assert_eq!(state.execute(&resources, &mut (), switch, 100001, 100001, arguments), Ok(value));
}

#[test]
fn call() {
    let id = Symbol::intern("id");
    let call = Symbol::intern("call");

    let mut prototypes = HashMap::new();
    prototypes.insert(id, ssa::Prototype::Script);
    prototypes.insert(call, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    let program = compile(&prototypes, "return argument0");
    resources.scripts.insert(id, program);

    let program = compile(&prototypes, "return id(3) + 5");
    resources.scripts.insert(call, program);

    let mut state = vm::State::new();

    let value = vm::Value::from(8);
    assert_eq!(state.execute(&resources, &mut (), call, 100001, 100001, &[]), Ok(value));
}

#[test]
fn array() {
    let array = Symbol::intern("array");

    let mut prototypes = HashMap::new();
    prototypes.insert(array, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    let program = compile(&prototypes, "{
        var a, b;
        a[0] = 3
        a[1] = 5
        b = 8
        b[2] = 13
        return a + a[1] + b[0] + b[1] + b[2]
    }");
    resources.scripts.insert(array, program);

    let mut state = vm::State::new();

    let value = vm::Value::from(29);
    assert_eq!(state.execute(&resources, &mut (), array, 100001, 100001, &[]), Ok(value));
}

#[test]
fn fibonacci() {
    let fibonacci = Symbol::intern("fibonacci");

    let mut prototypes = HashMap::new();
    prototypes.insert(fibonacci, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    let program = compile(&prototypes, "{
        var a, b, t;
        a = 0
        b = 1
        repeat (argument0) {
            t = a + b
            a = b
            b = t
        }
        return a
    }");
    resources.scripts.insert(fibonacci, program);

    let mut state = vm::State::new();

    let arguments = &[vm::Value::from(100)];
    let value = vm::Value::from(354224848179262000000.0);
    assert_eq!(state.execute(&resources, &mut (), fibonacci, 100001, 100001, arguments), Ok(value));
}

#[test]
fn memoize() {
    let fibonacci = Symbol::intern("fibonacci");

    let mut prototypes = HashMap::new();
    prototypes.insert(fibonacci, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    let program = compile(&prototypes, "{
        var i, fib;
        fib[0] = 0
        fib[1] = 1
        for (i = 2; i <= argument0; i += 1) {
            fib[i] = fib[i - 1] + fib[i - 2]
        }
        return fib[argument0]
    }");
    resources.scripts.insert(fibonacci, program);

    let mut state = vm::State::new();

    let arguments = &[vm::Value::from(100)];
    let value = vm::Value::from(354224848179262000000.0);
    assert_eq!(state.execute(&resources, &mut (), fibonacci, 100001, 100001, arguments), Ok(value));
}

#[test]
fn recurse() {
    let fibonacci = Symbol::intern("fibonacci");

    let mut prototypes = HashMap::new();
    prototypes.insert(fibonacci, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    let program = compile(&prototypes, "{
        if (argument0 < 2) {
            return 1
        } else {
            return fibonacci(argument0 - 1) + fibonacci(argument0 - 2)
        }
    }");
    resources.scripts.insert(fibonacci, program);

    let mut state = vm::State::new();

    let arguments = &[vm::Value::from(6)];
    let value = vm::Value::from(13);
    assert_eq!(state.execute(&resources, &mut (), fibonacci, 100001, 100001, arguments), Ok(value));
}

#[test]
fn ffi() {
    let add = Symbol::intern("add");
    let call = Symbol::intern("call");

    let mut prototypes = HashMap::new();
    prototypes.insert(add, ssa::Prototype::Function);
    prototypes.insert(call, ssa::Prototype::Script);

    let mut resources = vm::Resources::default();

    struct Context { value: f64 };
    impl Context {
        fn add(
            &mut self, state: &mut vm::State, _resources: &vm::Resources<Self>,
            _self_id: i32, _other_id: i32, arguments: vm::Arguments
        ) -> Result<vm::Value, vm::Error> {
            let arguments = state.arguments(arguments);
            let value = match (arguments[0].data(), arguments[1].data()) {
                (vm::Data::Real(a), vm::Data::Real(b)) => vm::Value::from(self.value + a + b),
                _ => vm::Value::from(0),
            };

            Ok(value)
        }
    }
    resources.functions.insert(add, Context::add);

    let program = compile(&prototypes, "return add(3, 5) + 8");
    resources.scripts.insert(call, program);

    let mut context = Context { value: 13.0 };
    let mut state = vm::State::new();

    let value = vm::Value::from(29);
    assert_eq!(state.execute(&resources, &mut context, call, 100001, 100001, &[]), Ok(value));
}
