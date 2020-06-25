use std::{slice, str, str::FromStr};

use project::{Action, action_kind, action_type, argument_type};

use crate::symbol::Symbol;
use crate::front::{ast, Lexer, Parser, Span, ErrorHandler};

pub struct ActionParser<'s, 'e> {
    reader: slice::Iter<'s, Action>,
    errors: &'e mut dyn ErrorHandler,

    current: Option<&'s Action>,
    span: Span,
}

impl<'s, 'e> ActionParser<'s, 'e> {
    pub fn new(
        reader: slice::Iter<'s, Action>,
        errors: &'e mut dyn ErrorHandler
    ) -> ActionParser<'s, 'e> {
        let mut parser = ActionParser {
            reader,
            errors,

            current: None,
            span: Span { low: 0, high: 0 },
        };

        parser.advance_action();
        parser
    }

    pub fn parse_event(&mut self) -> (ast::Action, Span) {
        let low = self.span.low;
        let mut high = low;

        let mut actions = vec![];
        while self.current.is_some() {
            let (action, span) = self.parse_action();
            actions.push((action, span));
            high = span.high;
        }

        let span = Span { low, high };
        (ast::Action::Block { body: actions.into_boxed_slice() }, span)
    }

    fn parse_action(&mut self) -> (ast::Action, Span) {
        let action_kind = match self.current {
            Some(action) => action.action_kind,
            None => {
                // Allow callers to expect an action when there are none left.
                // This enables cases like `[Question] [End of event]`.
                let span = Span { low: self.span.low, high: self.span.low };
                return (ast::Action::Block { body: Box::new([]) }, span)
            }
        };

        match action_kind {
            action_kind::NORMAL => self.parse_normal(),
            action_kind::BEGIN => self.parse_block(),
            action_kind::EXIT => self.parse_exit(),
            action_kind::REPEAT => self.parse_repeat(),
            action_kind::VARIABLE => self.parse_variable(),
            action_kind::CODE => self.parse_code(),

            action_kind::ELSE |
            action_kind::PLACEHOLDER |
            action_kind::SEPARATOR |
            action_kind::LABEL => {
                self.advance_action();
                self.parse_action()
            }

            _ => {
                let span = self.span;
                self.advance_action();

                self.errors.error(span, "unexpected action kind");
                (ast::Action::Error, span)
            }
        }
    }

    fn parse_normal(&mut self) -> (ast::Action, Span) {
        let low = self.span.low;
        let mut high = self.span.high;
        let mut offset = low;

        let action = self.current.unwrap();
        self.advance_action();

        let question = if action.is_question { Some(action.negate) } else { None };

        let execution = match action.action_type {
            action_type::FUNCTION => {
                let function = Symbol::intern(&action.name[..]);
                offset += action.name.len();

                ast::Exec::Function(function)
            }

            action_type::CODE => {
                let reader = Lexer::new(&action.code, offset);
                let mut parser = Parser::new(reader, self.errors);
                let program = Box::new(parser.parse_program());
                offset += action.code.len();

                ast::Exec::Code(program)
            }

            // advance_action skips comments, etc.; anything else is a corrupt action.
            _ => {
                let span = Span { low, high };
                return (ast::Action::Error, span)
            }
        };

        let target = if action.has_target { Some(action.target) } else { None };
        let relative = if action.has_relative { Some(action.relative) } else { None };

        let parameters = action.parameters.iter();
        let source = action.arguments.iter();
        let len = action.parameters_used as usize;
        let arguments: Vec<_> = Iterator::zip(parameters, source).take(len)
            .map(|(&param, source)| {
                let argument = self.parse_argument(param, source, offset);
                offset += source.len();

                argument
            })
            .collect();
        let arguments = arguments.into_boxed_slice();

        let question = question.map(|negate| {
            let (true_action, true_span) = self.parse_action();
            high = true_span.high;

            let false_action = match self.current {
                Some(Action { action_kind: action_kind::ELSE, .. }) => {
                    self.advance_action();

                    let (false_action, false_span) = self.parse_action();
                    high = false_span.high;

                    Some((false_action, false_span))
                }
                _ => None,
            };
            Box::new(ast::Question {
                negate,
                true_action: (true_action, true_span),
                false_action,
            })
        });

        let span = Span { low, high };
        (ast::Action::Normal { question, execution, target, relative, arguments }, span)
    }

    fn parse_argument(&mut self, param: u32, source: &[u8], offset: usize) -> ast::Argument {
        // Decode a valid UTF-8 prefix of the argument.
        // Anything after that would be ignored by integer parsing anyway.
        let source_str = str::from_utf8(source).unwrap_or_else(move |error| {
            let (valid, _) = source.split_at(error.valid_up_to());
            unsafe { str::from_utf8_unchecked(valid) }
        });

        let argument = match param {
            argument_type::EXPR => {
                let reader = Lexer::new(source, offset);
                let mut parser = Parser::new(reader, self.errors);
                ast::Argument::Expr(Box::new(parser.parse_expression(0)))
            }

            argument_type::STRING => {
                ast::Argument::String(Symbol::intern(source))
            }

            // Select EXPR or STRING based on whether the argument starts with a quote.
            argument_type::BOTH => {
                match source.first().copied() {
                    Some(b'"') => {
                        let reader = Lexer::new(source, offset);
                        let mut parser = Parser::new(reader, self.errors);
                        ast::Argument::Expr(Box::new(parser.parse_expression(0)))
                    }
                    _ => {
                        ast::Argument::String(Symbol::intern(source))
                    }
                }
            }

            argument_type::BOOL => {
                let value = u32::from_str(source_str);
                match value {
                    Ok(0) => ast::Argument::Bool(false),
                    Ok(1) => ast::Argument::Bool(true),
                    _ => ast::Argument::Error
                }
            }

            argument_type::MENU => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Menu).unwrap_or(ast::Argument::Error)
            }

            argument_type::SPRITE => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Sprite).unwrap_or(ast::Argument::Error)
            }

            argument_type::SOUND => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Sound).unwrap_or(ast::Argument::Error)
            }

            argument_type::BACKGROUND => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Background).unwrap_or(ast::Argument::Error)
            }

            argument_type::PATH => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Path).unwrap_or(ast::Argument::Error)
            }

            argument_type::SCRIPT => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Script).unwrap_or(ast::Argument::Error)
            }

            argument_type::OBJECT => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Object).unwrap_or(ast::Argument::Error)
            }

            argument_type::ROOM => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Room).unwrap_or(ast::Argument::Error)
            }

            argument_type::FONT => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Font).unwrap_or(ast::Argument::Error)
            }

            argument_type::COLOR => {
                let value = u32::from_str(source_str);
                value.map(ast::Argument::Color).unwrap_or(ast::Argument::Error)
            }

            argument_type::TIMELINE => {
                let value = i32::from_str(source_str);
                value.map(ast::Argument::Timeline).unwrap_or(ast::Argument::Error)
            }

            argument_type::FONT_STRING => {
                ast::Argument::FontString(Symbol::intern(source))
            }

            _ => ast::Argument::Error,
        };

        if let ast::Argument::Error = argument {
            let span = Span { low: offset, high: offset };
            self.errors.error(span, "corrupt argument");
        }

        argument
    }

    fn parse_block(&mut self) -> (ast::Action, Span) {
        let low = self.span.low;
        self.advance_action();

        let mut actions = vec![];
        loop {
            match self.current {
                Some(Action { action_kind: action_kind::END, .. }) | None => break,
                _ => (),
            }

            let (action, span) = self.parse_action();
            actions.push((action, span));
        }
        let body = actions.into_boxed_slice();

        let high = self.span.high;
        self.advance_action();

        let span = Span { low, high };
        (ast::Action::Block { body }, span)
    }

    fn parse_exit(&mut self) -> (ast::Action, Span) {
        let span = self.span;
        self.advance_action();
        (ast::Action::Exit, span)
    }

    fn parse_repeat(&mut self) -> (ast::Action, Span) {
        let low = self.span.low;
        let mut high = self.span.high;
        let offset = low;

        let action = self.current.unwrap();
        self.advance_action();

        if action.parameters_used != 1 {
            let span = Span { low, high };
            self.errors.error(span, "wrong number of arguments");
            return (ast::Action::Error, span);
        }

        let parameters = action.parameters.iter();
        let source = action.arguments.iter();
        let len = action.parameters_used as usize;
        let mut arguments = Iterator::zip(parameters, source).take(len);

        let (&parameter, source) = arguments.next().unwrap();
        let count = match parameter {
            argument_type::EXPR => {
                let reader = Lexer::new(source, offset);
                let mut parser = Parser::new(reader, self.errors);
                Box::new(parser.parse_expression(0))
            }

            _ => {
                let span = Span { low, high };
                self.errors.error(span, "expected an expression");
                Box::new((ast::Expr::Error, self.span))
            }
        };

        let (body, body_span) = self.parse_action();
        let body = Box::new((body, body_span));
        high = body_span.high;

        let span = Span { low, high };
        (ast::Action::Repeat { count, body }, span)
    }

    fn parse_variable(&mut self) -> (ast::Action, Span) {
        let low = self.span.low;
        let high = self.span.high;
        let offset = low;

        let action = self.current.unwrap();
        self.advance_action();

        if !action.has_target {
            let span = Span { low, high };
            self.errors.error(span, "expected a target");
            return (ast::Action::Error, span);
        }
        let target = action.target;

        if !action.has_relative {
            let span = Span { low, high };
            self.errors.error(span, "expected relative");
            return (ast::Action::Error, span);
        }
        let relative = action.relative;

        let len = action.parameters_used as usize;
        if len != 2 {
            let span = Span { low, high };
            self.errors.error(span, "wrong number of arguments");
            return (ast::Action::Error, span);
        }

        let parameters = action.parameters.iter();
        let source = action.arguments.iter();
        let mut arguments = Iterator::zip(parameters, source).take(len);

        let (&parameter, source) = arguments.next().unwrap();
        let variable = match parameter {
            argument_type::STRING => {
                let reader = Lexer::new(source, offset);
                let mut parser = Parser::new(reader, self.errors);
                Box::new(parser.parse_expression(0))
            }

            _ => {
                let span = Span { low, high };
                self.errors.error(span, "expected a variable");
                Box::new((ast::Expr::Error, self.span))
            }
        };

        let (&parameter, source) = arguments.next().unwrap();
        let value = match parameter {
            argument_type::EXPR => {
                let reader = Lexer::new(source, 0);
                let mut parser = Parser::new(reader, self.errors);
                Box::new(parser.parse_expression(0))
            }

            _ => {
                let span = Span { low, high };
                self.errors.error(span, "expected an expression");
                Box::new((ast::Expr::Error, self.span))
            }
        };

        let span = Span { low, high };
        (ast::Action::Variable { target, relative, variable, value }, span)
    }

    fn parse_code(&mut self) -> (ast::Action, Span) {
        let low = self.span.low;
        let high = self.span.high;
        let offset = low;

        let action = self.current.unwrap();
        self.advance_action();

        if !action.has_target {
            let span = Span { low, high };
            self.errors.error(span, "expected a target");
            return (ast::Action::Error, span);
        }
        let target = action.target;

        let len = action.parameters_used as usize;
        if len != 1 {
            let span = Span { low, high };
            self.errors.error(span, "wrong number of arguments");
            return (ast::Action::Error, span);
        }

        let parameters = action.parameters.iter();
        let source = action.arguments.iter();
        let mut arguments = Iterator::zip(parameters, source).take(len);

        let (&parameter, source) = arguments.next().unwrap();
        let code = match parameter {
            argument_type::STRING => {
                let reader = Lexer::new(source, offset);
                let mut parser = Parser::new(reader, self.errors);
                Box::new(parser.parse_program())
            }

            _ => {
                let span = Span { low, high };
                self.errors.error(span, "expected an expression");
                Box::new((ast::Stmt::Error(ast::Expr::Error), self.span))
            }
        };

        let span = Span { low, high };
        (ast::Action::Code { target, code }, span)
    }

    fn advance_action(&mut self) {
        loop {
            self.span = Span { low: self.span.high, high: self.span.high };
            self.current = self.reader.next();

            let action = match self.current {
                Some(action) => action,
                None => break,
            };
            self.span.high += match (action.action_kind, action.action_type) {
                (action_kind::NORMAL, action_type::FUNCTION) => action.name.len(),
                (action_kind::NORMAL, action_type::CODE) => action.code.len(),
                (_, _) => 0,
            };
            self.span.high += action.arguments[..action.parameters_used as usize].iter()
                .map(|argument| argument.len())
                .sum::<usize>();

            // Skip comments.
            match (action.action_kind, action.action_type) {
                (action_kind::NORMAL, action_type::NONE) => continue,
                (_, _) => break,
            }
        }
    }
}
