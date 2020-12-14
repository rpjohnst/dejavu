use gml::vm;
use wasm_bindgen::prelude::*;

pub type State = Closure<dyn FnMut()>;

pub struct Draw {
    pub canvas: JsValue,
}

impl Default for Draw {
    fn default() -> Self { Self { canvas: JsValue::UNDEFINED } }
}

pub fn run(mut cx: crate::Context) -> State {
    let mut thread = vm::Thread::default();

    crate::graphics::load(&mut cx);

    if let Err(error) = crate::room::State::load_room(&mut cx, &mut thread, 0) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    let callback: Box<dyn FnMut()> = Box::new(move || {
        crate::graphics::frame(&mut cx);
        crate::draw::State::draw_world(&mut cx);
        if let Err(error) = crate::instance::State::step(&mut cx, &mut thread) {
            let crate::World { show, .. } = &cx.world;
            show.show_vm_error(&*error);
        }
        crate::motion::State::simulate(&mut cx);
    });
    let closure = Closure::wrap(callback);
    start(&closure);
    closure
}

#[wasm_bindgen(module = "/src/platform/web.js")]
extern "C" {
    fn start(frame: &Closure<dyn FnMut()>) -> JsValue;
    pub fn stop();
}
