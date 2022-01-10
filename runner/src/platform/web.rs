use gml::vm;
use wasm::JsValue;

pub type State = impl FnMut();

#[derive(Default)]
pub struct Draw {
    pub canvas: JsValue,
}

pub fn run(mut cx: crate::Context) -> *mut State {
    let mut thread = vm::Thread::default();

    crate::graphics::load(&mut cx);

    if let Err(error) = crate::room::State::load_room(&mut cx, &mut thread, 0) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    if let Err(error) = crate::draw::State::draw(&mut cx, &mut thread) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    let frame = Box::new(move || {
        if let Err(error) = crate::instance::State::step(&mut cx, &mut thread) {
            let crate::World { show, .. } = &cx.world;
            show.show_vm_error(&*error);
        }
        crate::motion::State::simulate(&mut cx);

        if let Err(error) = crate::draw::State::draw(&mut cx, &mut thread) {
            let crate::World { show, .. } = &cx.world;
            show.show_vm_error(&*error);
        }
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

extern "system" {
    fn start(frame_fn: extern "system" fn(*mut u8), frame_cx: *mut u8);
    pub fn stop();
}
