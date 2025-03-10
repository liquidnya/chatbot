use crate::meta::{MetaCommandArgument, MetaCommandArguments};
use crate::pattern::CommandPattern;
use proc_macro2::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote_spanned;
use quote::ToTokens;

#[derive(Debug)]
pub enum Direction {
    Forwards,
    Backwards,
}

#[derive(Debug)]
pub struct CommandPatternToken<'a> {
    pattern: CommandPattern<'a>,
    /// ident and it's type span
    ident_span: Option<(Ident, Span)>,
    direction: Direction,
    /// span of the literal string
    span: Span,
}

impl<'a> CommandPatternToken<'a> {
    pub fn new(
        pattern: CommandPattern<'a>,
        direction: Direction,
        ident_span: Option<(Ident, Span)>,
        span: Span,
    ) -> Self {
        Self {
            pattern,
            ident_span,
            direction,
            span,
        }
    }
}

fn next<'a>(
    arguments: &'a MetaCommandArguments<'a>,
    direction: Direction,
    take_all: bool,
) -> MetaCommandArgument<'a> {
    match (direction, take_all) {
        (_, true) => arguments.next_rest(),
        (Direction::Forwards, false) => arguments.next(),
        (Direction::Backwards, false) => arguments.next_back(),
    }
}

impl CommandPatternToken<'_> {
    pub fn into_token_stream(self, arguments: &MetaCommandArguments<'_>) -> TokenStream {
        match self {
            CommandPatternToken {
                pattern: CommandPattern::Command(command),
                ident_span: None,
                direction,
                ..
            } => next(arguments, direction, false).to_match_command(command),
            CommandPatternToken {
                pattern: CommandPattern::Subcommand(subcommand),
                ident_span: None,
                direction,
                ..
            } => next(arguments, direction, false).to_match_subcommand(subcommand),
            CommandPatternToken {
                pattern:
                    CommandPattern::Argument {
                        name,
                        take_all,
                        optional,
                    },
                ident_span: Some((ident, span)),
                direction,
                ..
            } => {
                let next = next(arguments, direction, take_all);
                let next = if optional {
                    next.to_optional_argument(name)
                } else {
                    next.to_argument(name)
                };
                quote_spanned! {span=>
                    #[allow(non_snake_case)]
                    let #ident = #next;
                }
            }
            CommandPatternToken {
                pattern: CommandPattern::TakeAll,
                ident_span: None,
                span,
                ..
            } => {
                let next = arguments.next_rest().into_token_stream();
                quote_spanned! {span=>
                    #[allow(non_snake_case)]
                    #next;
                }
            }
            CommandPatternToken {
                pattern: CommandPattern::Argument { name, .. },
                ident_span: None,
                span,
                ..
            } => syn::Error::new(
                span,
                format!("`{}` can not be found in function arguments", name),
            )
            .to_compile_error(),
            token => syn::Error::new(
                token.span,
                format!("Unexpected error: No pattern for {:?}", token),
            )
            .to_compile_error(),
        }
    }
}

pub struct CommandPatternScanner<'a> {
    arguments: &'a MetaCommandArguments<'a>,
    optional: bool,
    take_all: bool,
}

impl<'a> CommandPatternScanner<'a> {
    pub fn new(arguments: &'a MetaCommandArguments<'a>) -> Self {
        Self {
            arguments,
            optional: false,
            take_all: false,
        }
    }
}

impl CommandPatternScanner<'_> {
    pub fn scan(&mut self, token: CommandPatternToken<'_>) -> Option<TokenStream> {
        let mut err = None;
        if token.pattern.is_optional() {
            self.optional = true;
        } else if self.optional {
            // last element is optional, but this one is not
            err = Some(
                syn::Error::new(
                    token.span,
                    format!("`{}` has to be optional", token.pattern),
                )
                .to_compile_error(),
            );
        }
        if self.take_all {
            // error, only one take all allowed and has to be the last item
            err = Some(
                syn::Error::new(
                    token.span,
                    format!("Only one `..` allowed in `{}`.", token.pattern),
                )
                .to_compile_error(),
            );
        }
        if token.pattern.is_taking_all() {
            self.take_all = true;
        }
        // generate code for the argument
        let mut command = token.into_token_stream(self.arguments);
        // extend with errors if there are any
        command.extend(err);
        // return result
        Some(command)
    }
}
