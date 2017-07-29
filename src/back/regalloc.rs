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
    vertices: Vec<ssa::Value>,
    adjacency: EntityMap<ssa::Value, Vec<ssa::Value>>,
}

impl Interference {
    /// Builds the interference graph for a function.
    ///
    /// Using a function's live value analysis, this algorithm determines which values are live at
    /// each definition point and marks them as interfering with the defined value.
    pub fn build(program: &ssa::Function, liveness: &Liveness) -> Interference {
        let mut vertices = Vec::with_capacity(program.values.len());
        let mut adjacency: EntityMap<_, Vec<_>> = EntityMap::with_capacity(program.values.len());

        for block in program.blocks.keys() {
            let mut live: HashSet<_> = liveness.out[block].clone();
            for &value in program.blocks[block].instructions.iter().rev() {
                live.remove(&value);

                vertices.push(value);
                adjacency[value].extend(live.iter().cloned());

                live.extend(program.uses(value));
            }

            let arguments = &program.blocks[block].arguments;
            for &value in arguments {
                vertices.push(value);
                adjacency[value].extend(arguments.into_iter().filter(|&&other| value != other));
            }
        }

        Interference { vertices, adjacency }
    }

    /// Colors the interference graph using an unbounded number of colors.
    ///
    /// This is a simple greedy algorithm to optimally color chordal graphs. It visits each node in
    /// a perfect elimination order, and assigns it the lowest color not used by any of its
    /// neighbors.
    pub fn color(self) -> (EntityMap<ssa::Value, usize>, usize) {
        let mut colors = EntityMap::with_capacity(self.adjacency.len());
        for &value in &self.vertices {
            colors[value] = usize::max_value();
        }

        let mut color_count = 0;
        for value in Self::perfect_elimination_order(self.vertices, &self.adjacency) {
            let mut neighbors = HashSet::with_capacity(self.adjacency[value].len());
            for &neighbor in &self.adjacency[value] {
                neighbors.insert(colors[neighbor]);
            }

            for color in 0.. {
                if !neighbors.contains(&color) {
                    colors[value] = color;
                    color_count = cmp::max(color_count, color + 1);
                    break;
                }
            }
        }

        (colors, color_count)
    }

    /// Computes a chordal graph's perfect elimination order using maximum cardinality search.
    ///
    /// See the `MaximumCardinalitySearch` iterator for details.
    fn perfect_elimination_order(
        vertices: Vec<ssa::Value>, adjacency: &EntityMap<ssa::Value, Vec<ssa::Value>>
    ) -> MaximumCardinalitySearch {
        let mut buckets = Vec::with_capacity(vertices.len());
        buckets.push(vertices.len());

        let weights = EntityMap::with_capacity(vertices.len());
        let mut indices = EntityMap::with_capacity(vertices.len());
        for (i, &value) in vertices.iter().enumerate() {
            indices[value] = i;
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
            for &neighbor in &self.adjacency[value] {
                let weight = self.weights[neighbor];
                let index = self.indices[neighbor];

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
