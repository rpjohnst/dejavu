/// Rectangle packer.
///
/// Uses the Skyline Bottom-Left heuristic.
pub struct Atlas {
    width: u16,
    height: u16,
    skyline: Vec<Segment>,
}

struct Segment { x: u16, y: u16 }

impl Atlas {
    pub fn new(width: u16, height: u16) -> Atlas {
        let mut atlas = Atlas { width: 0, height: 0, skyline: Vec::default() };
        atlas.reset(width, height);
        atlas
    }

    pub fn reset(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;

        self.skyline.clear();
        self.skyline.push(Segment { x: 0, y: 0 });
    }

    pub fn pack(&mut self, width: u16, height: u16) -> Option<(u16, u16)> {
        let mut position = 0;
        let mut bottom = u16::MAX;

        // Search for the lowest point on the skyline that can fit `width`.
        for i in 0..self.skyline.len() {
            let right = self.skyline[i].x + width;
            if right > self.width {
                break;
            }

            // Find the maximum height starting with segment `i` and spanning `width`.
            let top = self.skyline.iter()
                .skip(i).take_while(|&&Segment { x, .. }| x < right)
                .map(|&Segment { y, .. }| y).max()
                .unwrap_or(0);

            if top < bottom {
                position = i;
                bottom = top;
            }
        }
        if bottom + height > self.height {
            return None;
        }

        // Place a new segment on top of the skyline.
        let left = self.skyline[position].x;
        let right = left + width;
        self.skyline.insert(position, Segment { x: left, y: bottom + height });
        self.skyline[position + 1].x = right;

        // Remove old segments underneath the new segment.
        let rest = position + 2;
        let next = self.skyline.iter()
            .position(|&Segment { x, .. }| x > right)
            .unwrap_or(self.skyline.len());
        self.skyline.drain(rest..next);

        Some((left, bottom))
    }
}
