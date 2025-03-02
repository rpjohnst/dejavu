use std::ffi::c_char;
use gml::symbol::Symbol;
use gml::vm;
use wasm::JsValue;

pub type State = impl FnMut();

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

    let frame = Box::new(move || {
        if let Err(error) = crate::instance::State::step(&mut cx, &mut thread) {
            let crate::World { debug, .. } = &cx.world;
            debug.show_vm_error(&*error);
        }
        crate::motion::State::simulate(&mut cx);

        if let Err(error) = crate::draw::State::draw(&mut cx, &mut thread) {
            let crate::World { debug, .. } = &cx.world;
            debug.show_vm_error(&*error);
        }
        crate::draw::State::animate(&mut cx);
    });
    let frame_fn = invoke::<State>;
    let frame_cx = Box::into_raw(frame);
    unsafe { start(frame_fn, frame_cx as *mut _); }
    frame_cx
}

extern "system" fn invoke<F: FnMut()>(f: *mut u8) {
    let f = unsafe { &mut *(f as *mut F) };
    f()
}

unsafe extern "system" {
    fn start(frame_fn: extern "system" fn(*mut u8), frame_cx: *mut u8);
    pub fn stop();
}

pub struct Library;

impl Library {
    pub fn load(_dll: Symbol) -> Option<Library> { None }

    pub fn symbol(&self, _sym: *const c_char) -> Option<vm::Proc> { None }
}
