pub struct Batch {
    pub vertex: Vec<Vertex>,
    pub index: Vec<u16>,

    pub texture: i32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub image: [f32; 4],
}

pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32
}

impl Default for Batch {
    fn default() -> Batch {
        Batch {
            vertex: Vec::default(),
            index: Vec::default(),
            texture: -1,
        }
    }
}

impl Batch {
    pub fn reset(&mut self, texture: i32) {
        self.vertex.clear();
        self.index.clear();
        self.texture = texture;
    }

    pub fn quad(&mut self, position: Rect, uv: Rect, image: Rect) {
        let x1 = position.x;
        let y1 = position.y;
        let x2 = position.x + position.w;
        let y2 = position.y + position.h;

        let u1 = uv.x;
        let v1 = uv.y;
        let u2 = uv.x + uv.w;
        let v2 = uv.y + uv.h;

        let image = [image.x, image.y, image.w, image.h];

        let i = self.vertex.len() as u16;
        self.vertex.extend_from_slice(&[
            Vertex { position: [x1, y1, 0.0], uv: [u1, v1], image },
            Vertex { position: [x1, y2, 0.0], uv: [u1, v2], image },
            Vertex { position: [x2, y2, 0.0], uv: [u2, v2], image },
            Vertex { position: [x2, y1, 0.0], uv: [u2, v1], image },
        ]);
        self.index.extend_from_slice(&[
            i + 0, i + 1, i + 2,
            i + 0, i + 2, i + 3
        ]);
    }
}
