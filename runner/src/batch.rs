pub struct Batch {
    pub vertex: Vec<Vertex>,
    pub index: Vec<u16>,

    pub texture: i32,
    width: f32,
    height: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
    pub texture: [f32; 2],
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
            width: 0.0,
            height: 0.0,
        }
    }
}

impl Batch {
    pub fn reset(&mut self, texture: i32, (width, height): (u16, u16)) {
        self.vertex.clear();
        self.index.clear();
        self.texture = texture;
        self.width = width as f32;
        self.height = height as f32;
    }

    pub fn quad(&mut self, position: Rect, texture: Rect) {
        let x1 = position.x;
        let y1 = position.y;
        let x2 = position.x + position.w;
        let y2 = position.y + position.h;

        let u1 = texture.x / self.width;
        let v1 = texture.y / self.height;
        let u2 = (texture.x + texture.w) / self.width;
        let v2 = (texture.y + texture.h) / self.height;

        let i = self.vertex.len() as u16;
        self.vertex.extend_from_slice(&[
            Vertex { position: [x1, y2, 0.0], texture: [u1, v2], },
            Vertex { position: [x2, y2, 0.0], texture: [u2, v2], },
            Vertex { position: [x2, y1, 0.0], texture: [u2, v1], },
            Vertex { position: [x1, y1, 0.0], texture: [u1, v1], },
        ]);
        self.index.extend_from_slice(&[
            i + 0, i + 1, i + 2,
            i + 0, i + 2, i + 3
        ]);
    }
}
