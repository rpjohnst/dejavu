#![feature(test)]

extern crate gml;
extern crate test;

use std::path::PathBuf;

use gml::symbol::Symbol;
use gml::front::{self, SourceFile, ErrorHandler, Lexer, Parser};
use gml::back;
use gml::vm::{self, code};

fn compile(source: &str) -> code::Function {
    let source = SourceFile {
        name: PathBuf::from("<test>"),
        source: String::from(source),
    };
    let errors = ErrorHandler;
    let reader = Lexer::new(&source);
    let mut parser = Parser::new(reader, &errors);
    let program = parser.parse_program();
    let codegen = front::Codegen::new(&errors);
    let program = codegen.compile(&program);
    let codegen = back::Codegen::new();
    codegen.compile(&program)
}

#[test]
fn factorial() {
    let mut state = vm::State::new();

    let factorial = Symbol::intern("factorial");
    let program = compile("{ \
        var i, j; \
        j = 1 \
        for (i = 1; i <= 4; i += 1) \
            j *= i \
        return j \
    }");
    state.add_function(factorial, program);

    assert_eq!(state.execute(factorial, &[], 100001, 100001), Ok(vm::Value::from(24.0)));
}

#[test]
fn switch() {
    let mut state = vm::State::new();

    let switch = Symbol::intern("switch");
    let program = compile("{ \
        var i; \
        switch (argument0) { \
        case 3: \
            return 5 \
        case 8: \
            for (i = 0; i < 11; i += 1) {} \
            break \
        default: \
            return 15 \
        } \
        return i \
    }");
    state.add_function(switch, program);

    let value = vm::Value::from(5);
    assert_eq!(state.execute(switch, &[vm::Value::from(3)], 100001, 100001), Ok(value));

    let value = vm::Value::from(11);
    assert_eq!(state.execute(switch, &[vm::Value::from(8)], 100001, 100001), Ok(value));

    let value = vm::Value::from(15);
    assert_eq!(state.execute(switch, &[vm::Value::from(13)], 100001, 100001), Ok(value));
}

#[test]
fn call() {
    let mut state = vm::State::new();

    let id = Symbol::intern("id");
    let program = compile("return argument0");
    state.add_function(id, program);

    let call = Symbol::intern("call");
    let program = compile("return id(3) + 5");
    state.add_function(call, program);

    assert_eq!(state.execute(call, &[], 100001, 100001), Ok(vm::Value::from(8.0)));
}

#[test]
fn array() {
    let mut state = vm::State::new();

    let array = Symbol::intern("array");
    let program = compile("{ \
        var a, b; \
        a[0] = 3 \
        a[1] = 5 \
        b = 8 \
        b[2] = 13 \
        return a + a[1] + b[0] + b[1] + b[2] \
    }");
    state.add_function(array, program);

    assert_eq!(state.execute(array, &[], 100001, 100001), Ok(vm::Value::from(29.0)));
}

#[bench]
fn fibonacci(b: &mut test::Bencher) {
    let mut state = vm::State::new();

    let fibonacci = Symbol::intern("fibonacci");
    let program = compile("{ \
        var a, b, t; \
        a = 0 \
        b = 1 \
        repeat (argument0) { \
            t = a + b \
            a = b \
            b = t \
        } \
        return a \
    }");
    state.add_function(fibonacci, program);

    let value = vm::Value::from(354224848179262000000.0);
    b.iter(|| {
        assert_eq!(state.execute(fibonacci, &[vm::Value::from(100)], 100001, 100001), Ok(value))
    });
}

#[bench]
fn memoize(b: &mut test::Bencher) {
    let mut state = vm::State::new();

    let fibonacci = Symbol::intern("fibonacci");
    let program = compile("{ \
        var i, fib; \
        fib[0] = 0 \
        fib[1] = 1 \
        for (i = 2; i <= argument0; i += 1) { \
            fib[i] = fib[i - 1] + fib[i - 2] \
        } \
        return fib[argument0] \
    }");
    state.add_function(fibonacci, program);

    let value = vm::Value::from(354224848179262000000.0);
    b.iter(|| {
        assert_eq!(state.execute(fibonacci, &[vm::Value::from(100)], 100001, 100001), Ok(value))
    });
}

#[bench]
fn recurse(b: &mut test::Bencher) {
    let mut state = vm::State::new();

    let fibonacci = Symbol::intern("fibonacci");
    let program = compile("{ \
        if (argument0 < 2) { \
            return 1 \
        } else { \
            return fibonacci(argument0 - 1) + fibonacci(argument0 - 2) \
        } \
    }");
    state.add_function(fibonacci, program);

    let value = vm::Value::from(13);
    b.iter(|| {
        assert_eq!(state.execute(fibonacci, &[vm::Value::from(6)], 100001, 100001), Ok(value))
    });
}
