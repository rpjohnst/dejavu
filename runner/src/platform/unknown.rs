use gml::vm;

#[derive(Default)]
pub struct Draw;

pub fn run(mut cx: crate::Context) {
    let mut thread = vm::Thread::default();

    if let Err(error) = gml::vm::World::load(&mut cx, &mut thread) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    crate::graphics::load(&mut cx);

    if let Err(error) = crate::room::State::load_room(&mut cx, &mut thread, 0) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    if let Err(error) = crate::draw::State::draw(&mut cx, &mut thread) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }

    if let Err(error) = crate::instance::State::step(&mut cx, &mut thread) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }
    crate::motion::State::simulate(&mut cx);
}
