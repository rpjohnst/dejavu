use std::iter::{self, FromIterator};

use project::{action_kind, action_type};

pub mod token;
pub mod ast;
mod action_ast;

mod lexer;
mod parser;
mod action_parser;
mod ssa;
mod codegen;

pub use lexer::Lexer;
pub use parser::Parser;
pub use action_parser::ActionParser;
pub use codegen::Codegen;

/// A range of positions in an event or script.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Span {
    pub low: usize,
    pub high: usize,
}

/// Position data for an event or script.
///
/// Position data is hierarchical- an event is a sequence of actions, which is a sequence of
/// arguments, which is a sequence of lines, which is a sequence of columns.
///
/// Byte offsets for the start of each item are stored in sorted arrays. This allows us to map from
/// a single byte offset to user-facing action, argument, line, and column indexes with binary
/// search.
///
/// Converting an absolute index (relative to a whole event) into a local index (relative to the
/// parent item) works by subtracting the absolute index of the parent item's first child.
#[derive(Default)]
pub struct Lines {
    /// The byte offset of each action, and the absolute index of its first argument.
    pub actions: Vec<(usize, usize)>,
    /// The byte offset of each argument, and the absolute index of its first line.
    pub arguments: Vec<(usize, usize)>,
    /// The byte offset of each line. The absolute index of its first column has the same value.
    pub lines: Vec<usize>,
}

/// A user-facing position in an event or script.
pub struct Position {
    pub action: Option<usize>,
    pub argument: Option<usize>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl Lines {
    pub fn from_code(source: &[u8]) -> Lines {
        let actions = Vec::default();
        let arguments = Vec::default();
        let lines = Vec::from_iter(Self::compute_lines(source, 0));
        Lines { actions, arguments, lines }
    }

    pub fn from_actions(source: &[project::Action<'_>]) -> Lines {
        let mut actions = Vec::default();
        let mut arguments = Vec::default();
        let mut lines = Vec::default();

        let mut offset = 0;
        for action in source {
            actions.push((offset, arguments.len()));

            offset += match (action.action_kind, action.action_type) {
                (action_kind::NORMAL, action_type::FUNCTION) => action.name.len(),
                (action_kind::NORMAL, action_type::CODE) => action.code.len(),
                (_, _) => 0,
            };

            for argument in &action.arguments[..action.parameters_used as usize] {
                arguments.push((offset, lines.len()));

                if action.action_kind == action_kind::CODE {
                    lines.extend(Self::compute_lines(argument, offset));
                }

                offset += argument.len();
            }
        }

        Lines { actions, arguments, lines }
    }

    fn compute_lines(source: &[u8], offset: usize) -> impl Iterator<Item = usize> + '_ {
        let start = iter::once(offset);
        let newlines = source.iter().copied()
            .enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(move |(i, _)| offset + i + 1);
        Iterator::chain(start, newlines)
    }

    pub fn get_position(&self, pos: usize) -> Position {
        let action = self.actions.binary_search_by_key(&pos, |&(pos, _)| pos)
            .map(Some)
            .unwrap_or_else(|action| action.checked_sub(1));
        let argument = self.arguments.binary_search_by_key(&pos, |&(pos, _)| pos)
            .map(Some)
            .unwrap_or_else(|argument| argument.checked_sub(1));
        let line = self.lines.binary_search(&pos)
            .map(Some)
            .unwrap_or_else(|line| line.checked_sub(1));

        let action_pos = action.map(|action| self.actions[action]);
        let argument_pos = argument.map(|argument| self.arguments[argument]);
        let line_pos = line.map(|line| self.lines[line]);

        let action = match action {
            Some(action) => Some(1 + action),
            _ => None,
        };
        let argument = match (action_pos, argument) {
            (Some((_, first)), Some(argument)) if first <= argument => Some(1 + argument - first),
            _ => None,
        };
        let line = match (argument_pos, line) {
            (Some((_, first)), Some(line)) if first <= line => Some(1 + line - first),
            (_, Some(line)) => Some(1 + line),
            _ => None,
        };
        let column = match line_pos {
            Some(first) => Some(1 + pos - first),
            _ => None,
        };

        Position { action, argument, line, column }
    }
}
