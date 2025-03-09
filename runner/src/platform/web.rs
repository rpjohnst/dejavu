use std::ffi::c_char;
use gml::symbol::Symbol;
use gml::vm;
use wasm::JsValue;

pub struct State {
    cx: crate::Context,
    thread: vm::Thread,
    handle: u32,
}

#[derive(Default)]
pub struct Draw {
    pub canvas: JsValue,
}

pub fn run(mut cx: crate::Context) -> *mut State {
    let mut thread = vm::Thread::default();

    if let Err(error) = gml::vm::World::load(&mut cx, &mut thread) {
        let crate::World { debug, .. } = &cx.world;
        debug.show_vm_error(&*error);
    }

    crate::graphics::load(&mut cx);

    let room = cx.assets.room_order[0] as i32;
    if let Err(error) = crate::room::State::load_room(&mut cx, &mut thread, room) {
        let crate::World { debug, .. } = &cx.world;
        debug.show_vm_error(&*error);
    }

    if let Err(error) = crate::draw::State::draw(&mut cx, &mut thread) {
        let crate::World { debug, .. } = &cx.world;
        debug.show_vm_error(&*error);
    }
    crate::draw::State::animate(&mut cx);

    let frame_cx = Box::into_raw(Box::new(State { cx, thread, handle: 0 }));
    unsafe { (*frame_cx).handle = schedule(frame_fn, frame_cx); }
    frame_cx
}

extern "system" fn frame_fn(frame_cx: *mut State) {
    unsafe { (*frame_cx).handle = schedule(frame_fn, frame_cx); }
    let State { cx, thread, .. } = unsafe { &mut *frame_cx };

    if let Err(error) = crate::instance::State::step(cx, thread) {
        let crate::World { debug, .. } = &cx.world;
        debug.show_vm_error(&*error);
    }
    crate::motion::State::simulate(cx);

    if let Err(error) = crate::draw::State::draw(cx, thread) {
        let crate::World { debug, .. } = &cx.world;
        debug.show_vm_error(&*error);
    }
    crate::draw::State::animate(cx);
}

pub unsafe fn end(state: *mut State) {
    unsafe {
        let state = Box::from_raw(state);
        cancel(state.handle);
    }
}

unsafe extern "system" {
    #[allow(improper_ctypes)]
    fn schedule(frame_fn: extern "system" fn(*mut State), frame_cx: *mut State) -> u32;
    fn cancel(handle: u32);
}

pub struct Library;

impl Library {
    pub fn load(_dll: Symbol) -> Option<Library> { None }

    pub fn symbol(&self, _sym: *const c_char) -> Option<vm::Proc> { None }
}
