#[derive(Default)]
pub struct BitVec {
    data: Vec<u64>,
}

impl BitVec {
    pub fn new() -> BitVec {
        Self::default()
    }

    pub fn get(&self, bit: usize) -> bool {
        let word = bit / 64;
        let mask = 1 << (bit % 64);

        (self.data.get(word).unwrap_or(&0) & mask) != 0
    }

    pub fn set(&mut self, bit: usize) -> bool {
        self.ensure(bit);

        let word = bit / 64;
        let mask = 1 << (bit % 64);

        let result = (self.data[word] & mask) != 0;
        self.data[word] |= mask;
        result
    }

    pub fn clear(&mut self, bit: usize) -> bool {
        self.ensure(bit);

        let word = bit / 64;
        let mask = 1 << (bit % 64);

        let result = (self.data[word] & mask) != 0;
        self.data[word] &= !mask;
        result
    }

    fn ensure(&mut self, bit: usize) {
        let word = bit / 64;
        if word >= self.data.len() {
            self.data.resize(word + 1, 0);
        }
    }
}
