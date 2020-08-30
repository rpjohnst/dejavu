use std::ops::Range;
use std::marker::PhantomData;
use std::convert::{TryFrom, TryInto};
use crate::vm::{Thread, Result, Error, Entity, Value, ValueRef};

pub struct Bind<F, R, A, B>(pub F, pub PhantomData<fn(R, A) -> B>);

pub trait FnBind<'t, W> {
    const ARITY: usize;
    const VARIADIC: bool;
    unsafe fn call(self, cx: &mut W, thread: &'t mut Thread, args: Range<usize>) -> Result<Value>;
}
pub trait GetBind<W> {
    fn call(self, cx: &mut W, entity: Entity, i: usize) -> Value;
}
pub trait SetBind<'t, W> {
    fn call(self, cx: &mut W, entity: Entity, i: usize, value: ValueRef<'t>);
}

pub fn arity<'t, B: FnBind<'t, W>, W>(_: &B) -> usize { B::ARITY }
pub fn variadic<'t, B: FnBind<'t, W>, W>(_: &B) -> bool { B::VARIADIC }

pub trait Project<'r, R> { fn fields(&'r mut self) -> R; }

impl<W> Project<'_, ()> for W { fn fields(&mut self) -> () { () } }
impl<'r, W> Project<'r, (&'r mut W,)> for W { fn fields(&'r mut self) -> (&'r mut W,) { (self,) } }

pub trait IntoResult { fn into_result(self) -> Result<Value>; }

impl<R> IntoResult for R where Value: From<R> {
    fn into_result(self) -> Result<Value> { Ok(Value::from(self)) }
}
impl<R, E> IntoResult for std::result::Result<R, E> where Value: From<R>, Box<Error>: From<E> {
    fn into_result(self) -> Result<Value> {
        match self {
            Ok(val) => Ok(Value::from(val)),
            Err(err) => Err(Box::from(err)),
        }
    }
}

macro_rules! replace { ($x:tt, $($y:tt)*) => { $($y)* } }
macro_rules! count { ($($x:tt)*) => { <[()]>::len(&[$(replace!($x, ())),*]) } }

macro_rules! impl_fn_bind { (
    ($($r:ident)*) ($thread:ident $(: $t:ty)?) ($($e:ident)?) ($($p:ident: ($($b:tt)*)),*)
    ($args:ident[$range:ident]$(: $v:ty = $rest:expr)?)
) => {
    impl<'t, F, W, $($r,)* $($p,)* B> FnBind<'t, W> for
        Bind<F, ($(&mut $r,)*), ($(&mut $t,)? $($e,)? $($p,)* $($v,)?), B>
    where
        F: Fn($(&mut $r,)* $(&mut $t,)? $($e,)? $($p,)* $($v)?) -> B,
        W: for<'r> Project<'r, ($(&'r mut $r,)*)>,
        $($p: $($b)* + Default,)*
        B: IntoResult,
    {
        const ARITY: usize = count!($($p)*);
        const VARIADIC: bool = count!($($v)?) == 1;

        #[allow(nonstandard_style, unused, unreachable)]
        unsafe fn call(self, cx: &mut W, $thread: &'t mut Thread, range: Range<usize>) ->
            Result<Value>
        {
            let ($($r,)*) = cx.fields();
            $(let $e = $thread.self_entity();)?
            let args = $thread.arguments(range.clone());
            let ($($p,)* $args,) = match *args {
                [$(ref $p,)* ref ps @ ..] => ($($p.borrow().try_into().unwrap_or_default(),)* ps,),
                _ => return Err(Error::arity(args.len())),
            };
            let $range = range.start + count!($($p)*)..range.end;
            let Bind(api, _) = self;
            api($($r,)* $(replace!($t, $thread),)? $($e,)? $($p,)* $($rest)?).into_result()
        }
    }
} }

macro_rules! impl_get_bind { (($($r:ident)*) ($($e:ident)?) ($($i:ident)?)) => {
    impl<F, W, $($r,)* B> GetBind<W> for Bind<F, ($($r,)*), ($($e,)? $($i,)?), B> where
        F: Fn($(&$r,)* $($e,)? $($i,)?) -> B,
        W: for<'r> Project<'r, ($(&'r mut $r,)*)>,
        B: Into<Value>,
    {
        #[allow(nonstandard_style, unused, unreachable)]
        fn call(self, cx: &mut W, entity: Entity, index: usize) -> Value {
            let ($($r,)*) = cx.fields();
            $(let $e = entity;)?
            $(let $i = index;)?
            let Bind(api, _) = self;
            api($($r,)* $($e,)? $($i,)?).into()
        }
    }
} }

macro_rules! impl_set_bind { (($($r:ident)*) ($($e:ident)?) ($($i:ident)?)) => {
    impl<'t, F, W, $($r,)* P> SetBind<'t, W> for Bind<F, ($($r,)*), ($($e,)? $($i,)? P,), ()> where
        F: Fn($(&mut $r,)* $($e,)? $($i,)? P),
        W: for<'r> Project<'r, ($(&'r mut $r,)*)>,
        P: TryFrom<ValueRef<'t>> + Default,
    {
        #[allow(nonstandard_style, unused, unreachable)]
        fn call(self, cx: &mut W, entity: Entity, index: usize, value: ValueRef<'t>) {
            let ($($r,)*) = cx.fields();
            $(let $e = entity;)?
            $(let $i = index;)?
            let Bind(api, _) = self;
            api($($r,)* $($e,)? $($i,)? value.try_into().unwrap_or_default());
        }
    }
} }

macro_rules! impl_fn_bind_thread { ($rs:tt $e:tt ($($p:ident)*)) => {
    impl_fn_bind! {
        $rs (thread) $e ($($p: (TryFrom<ValueRef<'t>>)),*)
        (args[range])
    }
    impl_fn_bind! {
        $rs (thread) $e ($($p: (TryFrom<ValueRef<'t>>)),*)
        (args[range]: &[Value] = args)
    }
    impl_fn_bind! {
        $rs (thread: Thread) $e ($($p: (for<'u> TryFrom<ValueRef<'u>>)),*)
        (args[range])
    }
    impl_fn_bind! {
        $rs (thread: Thread) $e ($($p: (for<'u> TryFrom<ValueRef<'u>>)),*)
        (args[range]: Range<usize> = range)
    }
} }

macro_rules! impl_fn_bind_params { ($rs:tt $e:tt) => {
    impl_fn_bind_thread!($rs $e ());
    impl_fn_bind_thread!($rs $e (P0));
    impl_fn_bind_thread!($rs $e (P0 P1));
    impl_fn_bind_thread!($rs $e (P0 P1 P2));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13 P14));
    impl_fn_bind_thread!($rs $e (P0 P1 P2 P3 P4 P5 P6 P7 P8 P9 P10 P11 P12 P13 P14 P15));
} }

type Index = usize;
macro_rules! impl_bind_index { ($rs:tt $e:tt) => {
    impl_get_bind!($rs $e ());
    impl_get_bind!($rs $e (Index));
    impl_set_bind!($rs $e ());
    impl_set_bind!($rs $e (Index));
} }

macro_rules! impl_bind_self { ($rs:tt) => {
    impl_fn_bind_params!($rs ());
    impl_fn_bind_params!($rs (Entity));
    impl_bind_index!($rs ());
    impl_bind_index!($rs (Entity));
} }

impl_bind_self!(());
impl_bind_self!((R0));
impl_bind_self!((R0 R1));
