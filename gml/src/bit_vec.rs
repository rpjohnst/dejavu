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

    pub fn reset(&mut self, bit: usize) -> bool {
        self.ensure(bit);

        let word = bit / 64;
        let mask = 1 << (bit % 64);

        let result = (self.data[word] & mask) != 0;
        self.data[word] &= !mask;
        result
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    fn ensure(&mut self, bit: usize) {
        let word = bit / 64;
        if word >= self.data.len() {
            self.data.resize(word + 1, 0);
        }
    }
}

impl FromIterator<usize> for BitVec {
    fn from_iter<T: IntoIterator<Item = usize>>(iter: T) -> Self {
        let mut vec = BitVec::default();
        vec.extend(iter);
        vec
    }
}

impl Extend<usize> for BitVec {
    fn extend<T: IntoIterator<Item = usize>>(&mut self, iter: T) {
        for bit in iter { self.set(bit); }
    }
}
