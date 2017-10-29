use std::cmp;
use std::collections::HashSet;

use entity::EntityMap;
use back::ssa;
use back::analysis::*;

/// A value interference graph.
///
/// Used values are nodes in the graph, and values that are live simultaneously share an edge. This
/// means that values can be assigned non-interfering storage locations by coloring the graph so
/// that no two adjacent nodes share the same color.
pub struct Interference {
    params: Vec<ssa::Value>,
    vertices: Vec<ssa::Value>,
    adjacency: EntityMap<ssa::Value, Vec<ssa::Value>>,
}

impl Interference {
    /// Builds the interference graph for a function.
    ///
    /// Using a function's live value analysis, this algorithm determines which values are live at
    /// each definition point and marks them as interfering with the defined value.
    pub fn build(program: &ssa::Function, liveness: &Liveness) -> Interference {
        let mut params = Vec::new();
        let mut vertices = Vec::with_capacity(program.values.len());
        let mut adjacency: EntityMap<_, Vec<_>> = EntityMap::with_capacity(program.values.len());

        for block in program.blocks.keys() {
            let mut live: HashSet<_> = liveness.out[block].clone();
            for &value in program.blocks[block].instructions.iter().rev() {
                for def in program.defs(value) {
                    live.remove(&def);
                    vertices.push(def);

                    adjacency[def].extend(live.iter().cloned());
                    for &used in &live {
                        adjacency[used].push(def);
                    }
                }

                live.extend(program.uses(value));
            }

            // arguments to the entry block are actually program-level arguments
            let arguments = &program.blocks[block].arguments;
            if block == program.entry() {
                params.extend(arguments);
            } else {
                vertices.extend(arguments);
            }

            for &def in arguments {
                let live = live.iter().filter(|&&other| def != other);

                adjacency[def].extend(live.clone());
                for &used in live {
                    adjacency[used].push(def);
                }
            }
        }

        Interference { params, vertices, adjacency }
    }

    /// Colors the interference graph using an unbounded number of colors.
    ///
    /// This is a simple greedy algorithm to optimally color chordal graphs. It visits each node in
    /// a perfect elimination order, and assigns it the lowest color not used by any of its
    /// neighbors.
    pub fn color(self) -> (EntityMap<ssa::Value, usize>, usize, usize) {
        let mut colors = EntityMap::with_capacity(self.adjacency.len());
        for &value in &self.vertices {
            colors[value] = usize::max_value();
        }

        // Program arguments must be in order at the start of the stack frame
        // We know they all interfere with each other so there's no reason to run MCS on them.
        let param_count = self.params.len();
        for (color, &value) in self.params.iter().enumerate() {
            colors[value] = color;
        }

        let mut color_count = self.params.len();
        for value in Self::perfect_elimination_order(self.params, self.vertices, &self.adjacency) {
            let neighbors: HashSet<_> = self.adjacency[value].iter()
                .map(|&neighbor| colors[neighbor])
                .collect();

            for color in 0.. {
                if !neighbors.contains(&color) {
                    colors[value] = color;
                    color_count = cmp::max(color_count, color + 1);
                    break;
                }
            }
        }

        (colors, param_count, color_count)
    }

    /// Computes a chordal graph's perfect elimination order using maximum cardinality search.
    ///
    /// See the `MaximumCardinalitySearch` iterator for details.
    fn perfect_elimination_order(
        params: Vec<ssa::Value>, vertices: Vec<ssa::Value>,
        adjacency: &EntityMap<ssa::Value, Vec<ssa::Value>>
    ) -> MaximumCardinalitySearch {
        let mut buckets = Vec::with_capacity(vertices.len());
        buckets.push(vertices.len());

        let weights = EntityMap::with_capacity(adjacency.len());
        let mut indices = EntityMap::with_capacity(adjacency.len());
        for (i, &value) in vertices.iter().enumerate() {
            indices[value] = i;
        }

        // tell the algorithm that parameters have already been colored
        for (i, &value) in params.iter().enumerate() {
            indices[value] = vertices.len() + i;
        }

        MaximumCardinalitySearch {
            adjacency,

            vertices,
            buckets,
            weights,
            indices,
        }
    }
}

/// Maximum cardinality search iterator.
///
/// The MCS algorithm works by assigning a weight to each node in a chordal graph. It then
/// repeatedly takes the highest-weighted node and increments the weights of its neighbors.
///
/// The `vertices` array is split into buckets starting at the indices in `buckets`. Each node also
/// has its weight (or bucket index) stored in `weights`, and its index in `vertices` stored in
/// `indices`.
///
/// To increment a node's weight, the starting index of the next bucket is decremented, growing it
/// and shrinking the node's bucket by one. The node is then swapped into the new location. The end
/// of the `vertices` array thus always has a node from the highest-weighted bucket.
struct MaximumCardinalitySearch<'a> {
    adjacency: &'a EntityMap<ssa::Value, Vec<ssa::Value>>,

    vertices: Vec<ssa::Value>,
    buckets: Vec<usize>,
    weights: EntityMap<ssa::Value, usize>,
    indices: EntityMap<ssa::Value, usize>,
}

impl<'a> Iterator for MaximumCardinalitySearch<'a> {
    type Item = ssa::Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.vertices.pop().map(|value| {
            let weight = self.weights[value];
            self.buckets[weight] -= 1;
            self.buckets.truncate(weight + 1);

            for &neighbor in &self.adjacency[value] {
                let weight = self.weights[neighbor];
                let index = self.indices[neighbor];
                if index >= self.vertices.len() {
                    continue;
                }

                self.buckets[weight] -= 1;
                let bucket = self.buckets[weight];
                let other = self.vertices[bucket];

                self.vertices.swap(index, bucket);
                self.indices.swap(neighbor, other);

                self.weights[neighbor] += 1;
                if self.weights[neighbor] == self.buckets.len() {
                    self.buckets.push(self.vertices.len());
                }
            }

            value
        })
    }
}
