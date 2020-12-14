use std::slice;
use wasm_bindgen::prelude::*;

pub struct Draw {
    renderer: JsValue,
}

pub fn load(cx: &mut crate::Context) {
    let crate::Context { world, assets, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { platform, graphics, .. } = draw;

    let texture = &assets.textures[0];
    let (width, height) = texture.size;
    let renderer = renderer_new(&platform.canvas, &texture.data[..], width, height);

    *graphics = Some(Draw { renderer });
}

pub fn frame(cx: &mut crate::Context) {
    let crate::Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, .. } = draw;
    let Draw { renderer } = graphics.as_mut().unwrap();
    renderer_frame(renderer);
}

pub fn batch(cx: &mut crate::Context) {
    let crate::Context { world, .. } = cx;
    let crate::World { draw, .. } = world;
    let crate::draw::State { graphics, batch, .. } = draw;
    let Draw { renderer } = graphics.as_mut().unwrap();
    if batch.index.len() == 0 {
        return;
    }

    unsafe {
        let ptr = batch.vertex.as_ptr() as *const f32;
        let len = batch.vertex.len() * 5;
        let vertex = slice::from_raw_parts(ptr, len);
        let index = &batch.index[..];

        renderer_batch(&renderer, vertex, index);
    }
}

#[wasm_bindgen(module = "/src/graphics/webgl2.js")]
extern "C" {
    fn renderer_new(canvas: &JsValue, atlas: &[u8], width: u16, height: u16) -> JsValue;
    fn renderer_frame(renderer: &JsValue);
    fn renderer_batch(renderer: &JsValue, vertex_data: &[f32], index_data: &[u16]);
}
