use std::ops::Range;
use gml::{self, vm};

#[derive(Default)]
pub struct State;

#[gml::bind]
impl State {
    #[gml::api]
    pub fn script_execute(
        cx: &mut crate::Context, thread: &mut vm::Thread,
        scr: i32, args: Range<usize>
    ) -> vm::Result<vm::Value> {
        let scr = gml::Function::Script { id: scr };
        let args = Vec::from(unsafe { thread.arguments(args) });
        thread.execute(cx, scr, args)
    }

    #[gml::api]
    pub fn action_execute_script(
        cx: &mut crate::Context, thread: &mut vm::Thread, scr: i32, args: Range<usize>
    ) -> vm::Result<vm::Value> {
        Self::script_execute(cx, thread, scr, args)
    }
}
