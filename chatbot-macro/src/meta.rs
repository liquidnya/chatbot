use proc_macro2::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use quote::ToTokens;
use quote::TokenStreamExt;

pub struct MetaCommandRequest<'a> {
    ident: &'a Ident,
}

impl ToTokens for MetaCommandRequest<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident.to_tokens(tokens)
    }
}

impl<'a> MetaCommandRequest<'a> {
    pub fn new(ident: &'a Ident) -> Self {
        Self { ident }
    }
}

pub struct MetaCommandArguments<'a> {
    ident: &'a Ident,
}

impl ToTokens for MetaCommandArguments<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident.to_tokens(tokens)
    }
}

enum MetaCommandArgumentsFunction {
    Next,
    NextBack,
    NextRest,
}

impl ToTokens for MetaCommandArgumentsFunction {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(Ident::new(
            match self {
                MetaCommandArgumentsFunction::Next => "next",
                MetaCommandArgumentsFunction::NextBack => "next_back",
                MetaCommandArgumentsFunction::NextRest => "next_rest",
            },
            Span::call_site(),
        ))
    }
}

impl<'a> MetaCommandArguments<'a> {
    pub fn new(ident: &'a Ident) -> Self {
        Self { ident }
    }

    pub fn to_binding(&self, request: &MetaCommandRequest) -> TokenStream {
        quote! {
            let mut #self = ::chatbot_lib::command::CommandArguments::from(#request.command() as &str);
        }
    }

    pub fn to_empty_check(&self) -> TokenStream {
        let next_rest = self.next_rest();
        quote! {
            if (#next_rest.is_some()) {
                return Err(::chatbot_lib::command::CommandError::ArgumentsLeftOver);
            }
        }
    }

    pub fn next(&self) -> MetaCommandArgument<'_> {
        MetaCommandArgument {
            arguments: self,
            funtion: MetaCommandArgumentsFunction::Next,
        }
    }

    pub fn next_rest(&self) -> MetaCommandArgument<'_> {
        MetaCommandArgument {
            arguments: self,
            funtion: MetaCommandArgumentsFunction::NextRest,
        }
    }

    pub fn next_back(&self) -> MetaCommandArgument<'_> {
        MetaCommandArgument {
            arguments: self,
            funtion: MetaCommandArgumentsFunction::NextBack,
        }
    }
}
pub struct MetaCommandArgument<'a> {
    arguments: &'a MetaCommandArguments<'a>,
    funtion: MetaCommandArgumentsFunction,
}

impl MetaCommandArgument<'_> {
    pub fn to_argument(&self, name: &str) -> TokenStream {
        quote! {
            ::chatbot_lib::command::next_argument_anyhow(#self, #name)?
        }
    }

    pub fn to_optional_argument(&self, name: &str) -> TokenStream {
        quote! {
            ::chatbot_lib::command::next_optional_argument_anyhow(#self, #name)?
        }
    }

    pub fn to_match_subcommand(&self, subcommand: &str) -> TokenStream {
        quote! {
            if #self.ok_or(::chatbot_lib::command::CommandError::SubcommandMismatch)? != #subcommand {
                return Err(::chatbot_lib::command::CommandError::SubcommandMismatch);
            }
        }
    }

    pub fn to_match_command(&self, command: &str) -> TokenStream {
        quote! {
            if #self.ok_or(::chatbot_lib::command::CommandError::CommandMismatch)? != #command {
                return Err(::chatbot_lib::command::CommandError::CommandMismatch);
            }
        }
    }
}

impl ToTokens for MetaCommandArgument<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let arguments = &self.arguments;
        let function = &self.funtion;
        {
            quote! {
                #arguments.#function()
            }
        }
        .to_tokens(tokens);
    }
}
