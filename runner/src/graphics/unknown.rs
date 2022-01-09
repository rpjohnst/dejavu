pub struct Draw;

pub fn load(cx: &mut crate::Context) {
    let crate::Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, .. } = draw;

    *graphics = Some(Draw);
}

pub fn frame(cx: &mut crate::Context) {
    let crate::Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, .. } = draw;
    let Draw = graphics.as_mut().unwrap();
}

pub fn batch(cx: &mut crate::Context) {
    let crate::Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, .. } = draw;
    let Draw = graphics.as_mut().unwrap();
}

pub fn present(_cx: &mut crate::Context) {
}
