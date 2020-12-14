use gml::vm;

pub type State = ();

#[derive(Default)]
pub struct Draw;

pub fn run(mut cx: crate::Context) {
    let mut thread = vm::Thread::default();

    crate::graphics::load(&mut cx);

    if let Err(error) = crate::room::State::load_room(&mut cx, &mut thread, 0) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    crate::graphics::frame(&mut cx);
    crate::draw::State::draw_world(&mut cx);
}
