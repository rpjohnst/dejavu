use wasm::JsValue;

pub struct Draw {
    renderer: JsValue,
}

impl Drop for Draw {
    fn drop(&mut self) {
        if self.renderer != JsValue::UNDEFINED {
            unsafe { renderer_drop(self.renderer) };
        }
    }
}

pub fn load(cx: &mut crate::Context) {
    let crate::Context { world, assets, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { platform, graphics, .. } = draw;

    let texture = &assets.textures[0];
    let (width, height) = texture.size;
    let renderer = unsafe {
        let ptr = texture.data.as_ptr();
        let len = texture.data.len();
        renderer_new(platform.canvas, ptr, len, width, height)
    };

    *graphics = Some(Draw { renderer });
}

pub fn frame(cx: &mut crate::Context) {
    let crate::Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, .. } = draw;
    let &mut Draw { renderer } = graphics.as_mut().unwrap();
    unsafe { renderer_frame(renderer) };
}

pub fn batch(cx: &mut crate::Context) {
    let crate::Context { world, assets, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, batch, .. } = draw;
    let &mut Draw { renderer } = graphics.as_mut().unwrap();
    if batch.index.len() == 0 {
        return;
    }

    let atlas = &assets.textures[batch.texture as usize];
    let (width, height) = atlas.size;

    unsafe {
        let vertex_ptr = batch.vertex.as_ptr() as *const f32;
        let vertex_len = batch.vertex.len() * 9;
        let index_ptr = batch.index.as_ptr();
        let index_len = batch.index.len();

        renderer_batch(renderer, vertex_ptr, vertex_len, index_ptr, index_len, width, height);
    }
}

pub fn present(_cx: &mut crate::Context) {
}

unsafe extern "system" {
    fn renderer_new(
        canvas: JsValue,
        atlas_ptr: *const u8, atlas_len: usize, width: u16, height: u16
    ) -> JsValue;
    fn renderer_drop(renderer: JsValue);
    fn renderer_frame(renderer: JsValue);
    fn renderer_batch(
        renderer: JsValue,
        vertex_ptr: *const f32, vertex_len: usize,
        index_ptr: *const u16, index_len: usize,
        width: u16, height: u16
    );
}
