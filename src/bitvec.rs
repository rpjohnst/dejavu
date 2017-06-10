pub struct BitVec {
    data: Vec<u64>,
}

impl BitVec {
    pub fn new() -> BitVec {
        BitVec { data: Vec::new() }
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
