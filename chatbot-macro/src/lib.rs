#![deny(clippy::all)]

use indexmap::map::IndexMap;
use proc_macro::TokenStream;
use quote::quote;
use quote::quote_spanned;
use quote::{format_ident, ToTokens};
use syn::bracketed;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::Ident;
use syn::Path;
use syn::Type;
use syn::{punctuated::Punctuated, FnArg, Pat};

mod meta;
mod pattern;
mod rev_on;
mod token;

use meta::{MetaCommandArguments, MetaCommandRequest};
use pattern::CommandPattern;
use rev_on::RevOnIterator;
use token::{CommandPatternScanner, CommandPatternToken, Direction};

struct Argument<'a> {
    arg: String,
    ident: Ident,
    ty: &'a Type,
}

fn get_argument_names<T>(args: &Punctuated<FnArg, T>) -> syn::Result<Vec<Argument<'_>>> {
    let mut result = Vec::with_capacity(args.len());
    for arg in args {
        if let FnArg::Typed(arg) = arg {
            if let Pat::Ident(ref id) = *arg.pat {
                result.push(Argument {
                    arg: id.ident.to_string(),
                    ident: format_ident!("argument_{}", id.ident, span = id.ident.span()),
                    ty: &arg.ty,
                });
                continue;
            } else {
                return Err(syn::Error::new_spanned(&arg.pat, "Expected identifier"));
            }
        } else {
            return Err(syn::Error::new_spanned(
                &arg,
                "self is not allowed for this macro",
            ));
        }
    }
    Ok(result)
}

struct Commands {
    _struct_token: syn::Token![struct],
    ident: Ident,
    _brace_token: syn::token::Bracket,
    commands: syn::punctuated::Punctuated<CommandPath, syn::Token![,]>,
}

struct CommandPath {
    path: Path,
}

impl Parse for CommandPath {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        Ok(Self {
            path: input.call(Path::parse_mod_style)?,
        })
    }
}

impl Parse for Commands {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let content;
        let struct_token = input.parse()?;
        let ident = input.parse()?;
        let brace_token = bracketed!(content in input);
        let commands = content
            .call(syn::punctuated::Punctuated::<CommandPath, syn::Token![,]>::parse_terminated)?;
        Ok(Self {
            _struct_token: struct_token,
            ident,
            _brace_token: brace_token,
            commands,
        })
    }
}

#[proc_macro]
pub fn commands(item: TokenStream) -> TokenStream {
    let commands = syn::parse_macro_input!(item as Commands);
    let name = commands.ident;
    let commands = commands.commands.into_iter().map(|command| {
        let span = command.path.span();
        let command = command.path;
        let command_str = command
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<String>>()
            .join("::");
        let mut show_syntax = command.clone();
        if let Some(id) = show_syntax.segments.last_mut() {
            id.ident = format_ident!("show_syntax_{}", id.ident);
        }
        let mut command = command;
        if let Some(id) = command.segments.last_mut() {
            id.ident = format_ident!("async_command_{}", id.ident);
        }
        quote_spanned! {span=>
            match #command (request).await {
                response @ Ok(_) => {
                    log::debug!("Calling {}", #command_str);
                    return response.ok();
                },
                Err(e) => {
                    if #show_syntax.0 {
                        if e.is_argument_error() {
                            return Some(::chatbot_lib::response::Response::new(format!("{} {}", ::chatbot_lib::user::UserArgument::from(request.sender() as &User), #show_syntax.1)));
                        } else if e.is_subcommand_mismatch() {
                            if let Some(shared_syntax) = &mut shared_syntax {
                                shared_syntax.append(#show_syntax.1);
                            } else {
                                shared_syntax = Some(::chatbot_lib::command::FindSharedSyntax::new(#show_syntax.1));
                            }
                        }
                    }
                    log::debug!("Error calling {}: {:?}", #command_str, e)
                },
            };
        }
    });
    let code = quote! {
        struct #name;

        #[async_trait]
        impl CommandProcessor for #name {
            async fn process<'a>(&self, request: &'a CommandRequest<'a>) -> Option<Response<'a>> {
                let mut shared_syntax : Option<::chatbot_lib::command::FindSharedSyntax> = None;
                #(#commands)*
                if let Some(shared_syntax) = shared_syntax {
                    // TODO: use Display instead of ToString
                    return Some(::chatbot_lib::response::Response::new(format!("{} {}", ::chatbot_lib::user::UserArgument::from(request.sender() as &User), shared_syntax.to_string())));
                }
                None
            }
        }
    };
    code.into()
}

enum MetaArguments {
    Arguments(Punctuated<syn::MetaNameValue, syn::Token![,]>),
    Str(syn::LitStr),
}

impl ToTokens for MetaArguments {
    fn to_tokens(&self, stream: &mut proc_macro2::TokenStream) {
        match self {
            Self::Arguments(args) => args.to_tokens(stream),
            Self::Str(lit) => lit.to_tokens(stream),
        }
    }
}

impl Parse for MetaArguments {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        if input.peek(syn::LitStr) {
            input.parse().map(MetaArguments::Str)
        } else {
            Ok(MetaArguments::Arguments(input.call(
                syn::punctuated::Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated,
            )?))
        }
    }
}

fn get_str_argument<'a>(
    args: &'a MetaArguments,
    name: &str,
) -> Option<Result<&'a syn::LitStr, syn::Error>> {
    match args {
        MetaArguments::Arguments(args) => args
            .iter()
            .find(|arg| arg.path.is_ident(name))
            .map(|arg| &arg.lit)
            .map(|lit| {
                if let syn::Lit::Str(str) = lit {
                    Ok(str)
                } else {
                    Err(syn::Error::new_spanned(
                        &lit,
                        format!("expected a string literal for `{}`", name),
                    ))
                }
            }),
        MetaArguments::Str(str) if name == "pattern" => Some(Ok(str)),
        _ => None,
    }
}

fn get_bool_argument<'a>(
    args: &'a MetaArguments,
    name: &str,
) -> Option<Result<&'a syn::LitBool, syn::Error>> {
    match args {
        MetaArguments::Arguments(args) => args
            .iter()
            .find(|arg| arg.path.is_ident(name))
            .map(|arg| &arg.lit)
            .map(|lit| {
                if let syn::Lit::Bool(bool) = lit {
                    Ok(bool)
                } else {
                    Err(syn::Error::new_spanned(
                        &lit,
                        format!("expected a string literal for `{}`", name),
                    ))
                }
            }),
        _ => None,
    }
}

#[proc_macro_attribute]
pub fn command(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let is_async = input.sig.asyncness.is_some();
    let vis = &input.vis;
    let name = &input.sig.ident;

    // function arguments
    let arguments = input.clone().sig.inputs;
    let args = get_argument_names(&arguments);
    let fn_args = match args {
        Ok(args) => args,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    // command template and arguments
    let meta_arguments = syn::parse_macro_input!(attr as MetaArguments); // TODO: add option to use Rust syntax instead of using a LitStr

    let command_literal = get_str_argument(&meta_arguments, "pattern").unwrap_or_else(|| {
        Err(syn::Error::new_spanned(
            &meta_arguments,
            "the key `pattern` is required",
        ))
    });

    let command_literal = match command_literal {
        Err(e) => return e.to_compile_error().into(),
        Ok(str) => str,
    };

    let show_syntax_default = syn::LitBool {
        value: false,
        span: proc_macro2::Span::call_site(),
    };
    let show_syntax =
        get_bool_argument(&meta_arguments, "show_syntax").unwrap_or(Ok(&show_syntax_default));
    let show_syntax = match show_syntax {
        Err(e) => return e.to_compile_error().into(),
        Ok(value) => value,
    };
    let result_default = syn::LitBool {
        value: false,
        span: proc_macro2::Span::call_site(),
    };
    let result = get_bool_argument(&meta_arguments, "result").unwrap_or(Ok(&result_default));
    let result = match result {
        Err(e) => return e.to_compile_error().into(),
        Ok(value) => value,
    };

    let command_template = command_literal.value();
    let mut command_args: IndexMap<CommandPattern, Option<&Argument>> = command_template
        .split_whitespace()
        .map(Into::into)
        .map(|c| (c, None))
        .collect();
    let function_call = fn_args.iter().map(|arg| {
        let mut ident = arg.ident.clone();
        ident.set_span(arg.ty.span());
        ident
    });

    let function_call = if result.value {
        if is_async {
            quote! {
                let result = async move {
                    let result = #name(#(#function_call),*).await;
                    result.map(|result|::chatbot_lib::response::IntoResponse::into_response(result, request))
                };
                Ok(result)
            }
        } else {
            quote! {
                let result = #name(#(#function_call),*);
                Ok(result.map(|result|::chatbot_lib::response::IntoResponse::into_response(result, request)))
            }
        }
    } else if is_async {
        quote! {
            let result = async move {
                let result = #name(#(#function_call),*).await;
                ::chatbot_lib::response::IntoResponse::into_response(result, request)
            };
            Ok(result)
        }
    } else {
        quote! {
            let result = #name(#(#function_call),*);
            Ok(::chatbot_lib::response::IntoResponse::into_response(result, request))
        }
    };
    let return_type = if result.value {
        if is_async {
            quote!(
                impl core::future::Future<
                    Output = Result<
                        ::chatbot_lib::response::Response<'s>,
                        ::chatbot_lib::command::CommandError<anyhow::Error>,
                    >,
                > + 's
            )
        } else {
            quote!(
                Result<
                    ::chatbot_lib::response::Response<'s>,
                    ::chatbot_lib::command::CommandError<anyhow::Error>,
                >
            )
        }
    } else if is_async {
        quote!(impl core::future::Future<Output = ::chatbot_lib::response::Response<'s>> + 's)
    } else {
        quote!(::chatbot_lib::response::Response<'s>)
    };

    // match function arguments with command arguments
    let mut argument_parsers = quote! {};
    for arg in fn_args.iter() {
        let name = &arg.arg;
        if let Some(item) = command_args.get_mut(name.as_str()) {
            if item.replace(arg).is_some() {
                return syn::Error::new_spanned(
                    &arg.ident,
                    format!("Unexpected error: `{}` already defined.", name),
                )
                .to_compile_error()
                .into();
            }
        } else {
            let ident = &arg.ident;
            argument_parsers.extend(quote_spanned! {arg.ty.span()=>
                #[allow(non_snake_case)]
                let #ident = ::chatbot_lib::command::from_command_request_anyhow(&request)?;
            });
        }
    }

    let command_arguments = format_ident!("iter");
    let command_request = format_ident!("request");
    let command_arguments = MetaCommandArguments::new(&command_arguments);

    // command parsing
    let command_parser = command_args
        .into_iter()
        .map(|(pattern, ident_span)| {
            (
                pattern,
                ident_span.map(|args| (args.ident.clone(), args.ty.span())),
            )
        })
        .rev_on(|(pattern, _)| pattern.is_taking_all())
        .map(|((pattern, ident_span), rev)| {
            CommandPatternToken::new(
                pattern,
                if rev {
                    Direction::Backwards
                } else {
                    Direction::Forwards
                },
                ident_span,
                command_literal.span(),
            )
        })
        .scan(
            CommandPatternScanner::new(&command_arguments),
            CommandPatternScanner::scan,
        );

    let call_name = format_ident!("command_{}", name);
    let command_name = format_ident!("async_command_{}", name);
    let show_syntax_name = format_ident!("show_syntax_{}", name);
    let function_call2 = if result.value {
        if is_async {
            quote! {
                match #call_name (request) {
                    Ok(future) => future.await,
                    Err(e) => Err(e),
                }
            }
        } else {
            quote! {
                match #call_name (request) {
                    Ok(result) => result,
                    Err(e) => Err(e),
                }
            }
        }
    } else if is_async {
        quote! {
            match #call_name (request) {
                Ok(future) => Ok(future.await),
                Err(e) => Err(e),
            }
        }
    } else {
        quote! {
            #call_name (request)
        }
    };

    let command_arguments_binding =
        command_arguments.to_binding(&MetaCommandRequest::new(&command_request));
    let command_arguments_check = command_arguments.to_empty_check();

    // TODO: return type could be Either<Result<Response, CommandError>, impl Future<Oputput=Result<Response, CommandError>>>
    let result = quote! {
        #input

        fn #call_name<'s, 'a: 's, 'req: 's>(#command_request: &'a ::chatbot_lib::request::CommandRequest<'req>) -> Result<#return_type, ::chatbot_lib::command::CommandError<anyhow::Error>> {
            // convert request to function arguments
            #argument_parsers
            #command_arguments_binding
            // parse command arguments
            #(#command_parser)*
            #command_arguments_check


            #function_call
        }

        #vis async fn #command_name<'s, 'a: 's, 'req: 's>(request: &'a ::chatbot_lib::request::CommandRequest<'req>) -> Result<::chatbot_lib::response::Response<'s>, ::chatbot_lib::command::CommandError<anyhow::Error>> {
            #function_call2
        }

        #[allow(non_upper_case_globals)]
        #vis const #show_syntax_name: (bool, &'static str) = (#show_syntax, #command_literal);
    };
    result.into()
}
