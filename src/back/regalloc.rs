use std::cmp;
use std::collections::HashSet;

use handle_map::HandleMap;
use back::{ssa, analysis::*};

/// A value interference graph.
///
/// Used values are nodes in the graph, and values that are live simultaneously share an edge. This
/// means that values can be assigned non-interfering storage locations by coloring the graph so
/// that no two adjacent nodes share the same color.
pub struct Interference {
    adjacency: HandleMap<ssa::Value, Vec<ssa::Value>>,
    vertices: Vec<ssa::Value>,

    precolored: Vec<ssa::Value>,
    groups: Vec<usize>,
}

impl Interference {
    /// Builds the interference graph for a function.
    ///
    /// Using a function's live value analysis, this algorithm determines which values are live at
    /// each definition point and marks them as interfering with the defined value.
    pub fn build(program: &ssa::Function, liveness: &Liveness) -> Interference {
        let mut adjacency: HandleMap<_, Vec<_>> = HandleMap::with_capacity(program.values.len());
        let mut vertices = Vec::with_capacity(program.values.len());

        let mut precolored = Vec::new();
        let mut groups = Vec::new();

        let defs = &program.blocks[ssa::ENTRY].parameters;
        groups.push(precolored.len());
        precolored.extend(defs);
        if precolored.len() == 0 {
            precolored.push(program.return_def);
        }

        for block in program.blocks.keys() {
            let mut live: HashSet<_> = liveness.out[block].clone();
            for &value in program.blocks[block].instructions.iter().rev() {
                // values defined by the instruction
                let defs = program.defs(value);
                vertices.extend(defs);
                for def in defs {
                    live.remove(&def);

                    adjacency[def].extend(live.iter().cloned());
                    for &used in &live {
                        adjacency[used].push(def);
                    }
                }

                // values defined inside the instruction
                let defs = program.internal_defs(value);
                if defs.len() > 0 {
                    groups.push(precolored.len());
                    precolored.extend(defs);
                }
                for &def in defs {
                    adjacency[def].extend(live.iter().cloned());
                    for &used in &live {
                        adjacency[used].push(def);
                    }
                }

                // values used by the instruction
                live.extend(program.uses(value));
            }

            // parameters to the entry block are precolored
            if block == ssa::ENTRY {
                continue;
            }

            let defs = &program.blocks[block].parameters;
            vertices.extend(defs);
            for &def in defs {
                live.remove(&def);

                adjacency[def].extend(live.iter().cloned());
                for &used in &live {
                    adjacency[used].push(def);
                }
            }
        }

        groups.push(precolored.len());

        Interference { adjacency, vertices, precolored, groups }
    }

    /// Colors the interference graph using an unbounded number of colors.
    ///
    /// This is a simple greedy algorithm to optimally color chordal graphs. It visits each node in
    /// a perfect elimination order, and assigns it the lowest color not used by any of its
    /// neighbors.
    ///
    /// It also precolors program parameters and call arguments to match the VM's calling
    /// convention, with parameters at the start of the frame and arguments at the end.
    pub fn color(self) -> (HandleMap<ssa::Value, usize>, usize, usize) {
        let mut colors = HandleMap::with_capacity(self.adjacency.len());
        for &value in Iterator::chain(self.vertices.iter(), self.precolored.iter()) {
            colors[value] = usize::max_value();
        }

        let mut color_count;
        let param_count;

        // Program parameters must be in order at the start of the stack frame.
        let (start, end) = (self.groups[0], self.groups[1]);
        let parameters = &self.precolored[start..end];
        for (color, &value) in Iterator::zip(0.., parameters) {
            colors[value] = color;
        }
        color_count = parameters.len();
        param_count = color_count;

        // Regular values are allocated greedily in perfect/simplical elimination order
        for value in Self::perfect_elimination_order(&self.adjacency, self.vertices, &self.precolored) {
            let neighbors: HashSet<_> = self.adjacency[value].iter()
                .map(|&neighbor| colors[neighbor])
                .collect();
            let color = (0..)
                .skip_while(|&color| neighbors.contains(&color))
                .next()
                .unwrap_or(color_count);

            colors[value] = color;
            color_count = cmp::max(color_count, color + 1);
        }

        // Call arguments must be in order at the end of the (live) stack frame.
        for group in self.groups[1..].windows(2) {
            let (start, end) = (group[0], group[1]);
            let arguments = &self.precolored[start..end];

            let neighbors: HashSet<_> = self.adjacency[arguments[0]].iter()
                .map(|&neighbor| colors[neighbor])
                .collect();
            let color = (0..color_count).rev()
                .take_while(|&color| !neighbors.contains(&color))
                .last()
                .unwrap_or(color_count);

            for (color, &value) in Iterator::zip(color.., arguments) {
                colors[value] = color;
            }
            color_count = cmp::max(color_count, color + arguments.len());
        }

        (colors, param_count, color_count)
    }

    /// Computes a chordal graph's perfect elimination order using maximum cardinality search.
    ///
    /// See the `MaximumCardinalitySearch` iterator for details.
    fn perfect_elimination_order<'a>(
        adjacency: &'a HandleMap<ssa::Value, Vec<ssa::Value>>,
        vertices: Vec<ssa::Value>, precolored: &[ssa::Value]
    ) -> MaximumCardinalitySearch<'a> {
        let mut buckets = Vec::with_capacity(vertices.len());
        buckets.push(vertices.len());

        // construct the buckets with precolored values excluded
        let weights = HandleMap::with_capacity(adjacency.len());
        let mut indices = HandleMap::with_capacity_default(adjacency.len(), vertices.len());
        for (i, &value) in vertices.iter().enumerate() {
            indices[value] = i;
        }

        // increment the neighbors of precolored values
        let mut buckets = Buckets { vertices, buckets, weights, indices };
        for &value in precolored {
            for &neighbor in &adjacency[value] {
                buckets.increment(neighbor);
            }
        }

        MaximumCardinalitySearch { adjacency, buckets }
    }
}

/// Maximum cardinality search iterator.
///
/// The MCS algorithm works by assigning a weight to each node in a chordal graph. It then
/// repeatedly takes the highest-weighted node and increments the weights of its neighbors.
struct MaximumCardinalitySearch<'a> {
    adjacency: &'a HandleMap<ssa::Value, Vec<ssa::Value>>,
    buckets: Buckets,
}

impl<'a> Iterator for MaximumCardinalitySearch<'a> {
    type Item = ssa::Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.buckets.pop().map(|value| {
            for &neighbor in &self.adjacency[value] {
                self.buckets.increment(neighbor);
            }

            value
        })
    }
}

/// A collection of values sorted into buckets.
///
/// The `vertices` array is split into buckets starting at the indices in `buckets`. Each node also
/// has its weight (or bucket index) stored in `weights`, and its index in `vertices` stored in
/// `indices`.
struct Buckets {
    vertices: Vec<ssa::Value>,
    buckets: Vec<usize>,
    weights: HandleMap<ssa::Value, usize>,
    indices: HandleMap<ssa::Value, usize>,
}

impl Buckets {
    fn pop(&mut self) -> Option<ssa::Value> {
        self.vertices.pop().map(|value| {
            let weight = self.weights[value];
            self.buckets[weight] -= 1;
            self.buckets.truncate(weight + 1);

            value
        })
    }

    /// Increment a node's weight, moving it into the next bucket.
    ///
    /// To increment a node's weight, the starting index of the next bucket is decremented, growing
    /// it and shrinking the node's bucket by one. The node is then swapped into the new location.
    /// The end of the `vertices` array thus always has a node from the highest-weighted bucket.
    fn increment(&mut self, value: ssa::Value) {
        let weight = self.weights[value];
        let index = self.indices[value];
        if index >= self.vertices.len() {
            return;
        }

        self.buckets[weight] -= 1;
        let bucket = self.buckets[weight];
        let other = self.vertices[bucket];

        self.vertices.swap(index, bucket);
        self.indices.swap(value, other);

        self.weights[value] += 1;
        if self.weights[value] == self.buckets.len() {
            self.buckets.push(self.vertices.len());
        }
    }
}
