mod command_processor;
mod error;
mod from_argument;
mod split;
mod subcommand;

pub use self::command_processor::CommandProcessor;
pub use self::error::CommandError;
pub use self::from_argument::FromArgument;
pub use self::split::CommandArguments;
pub use self::subcommand::FindSharedSyntax;

use crate::request::{CommandRequest, FromCommandRequest};
use core::fmt::Debug;

pub fn next_argument<'req, T: FromArgument<'req> + 'req>(
    arg: Option<&'req str>,
    name: &'static str,
) -> Result<T, CommandError<<T as FromArgument<'req>>::Error>> {
    let to_parsing = move |err| -> CommandError<<T as FromArgument<'req>>::Error> {
        CommandError::NamedArgumentParsing(name, err)
    };
    match arg {
        None => Err(CommandError::ArgumentMissing),
        Some(arg) => {
            let arg = <T as FromArgument>::from_argument(arg);
            arg.map_err(to_parsing)
        }
    }
}

pub fn next_argument_dyn<'req, T: FromArgument<'req> + 'req>(
    arg: Option<&'req str>,
    name: &'static str,
) -> Result<T, CommandError<Box<dyn std::fmt::Debug + 'req>>> {
    next_argument(arg, name).map_err(|err| err.dyn_err())
}

pub fn next_argument_unit<'req, T: FromArgument<'req> + 'req>(
    arg: Option<&'req str>,
    name: &'static str,
) -> Result<T, CommandError<()>> {
    next_argument(arg, name).map_err(|err| err.unit_err())
}

pub fn next_optional_argument_unit<'req, T: FromArgument<'req> + 'req>(
    arg: Option<&'req str>,
    name: &'static str,
) -> Result<Option<T>, CommandError<()>> {
    match next_argument(arg, name) {
        Ok(value) => Ok(Some(value)),
        Err(CommandError::ArgumentMissing) => Ok(None),
        Err(error) => Err(error.unit_err()),
    }
}

pub fn next_argument_anyhow<'req, T: FromArgument<'req> + 'req>(
    arg: Option<&'req str>,
    name: &'static str,
) -> Result<T, CommandError<anyhow::Error>> {
    next_argument(arg, name).map_err(|err| err.map_err(anyhow::Error::new))
}

pub fn next_optional_argument_anyhow<'req, T: FromArgument<'req> + 'req>(
    arg: Option<&'req str>,
    name: &'static str,
) -> Result<Option<T>, CommandError<anyhow::Error>> {
    match next_argument(arg, name) {
        Ok(value) => Ok(Some(value)),
        Err(CommandError::ArgumentMissing) => Ok(None),
        Err(err) => Err(err.map_err(anyhow::Error::new)),
    }
}

pub fn from_command_request_dyn<'a, T: FromCommandRequest<'a, 'a> + 'a>(
    request: &'a CommandRequest<'a>,
) -> Result<T, Box<dyn Debug + 'a>> {
    let value = <T as FromCommandRequest>::from_command_request(request);
    value.map_err(|err| -> Box<dyn Debug + 'a> { Box::new(err) })
}

pub fn from_command_request_option<'a, T: FromCommandRequest<'a, 'a> + 'a>(
    request: &'a CommandRequest<'a>,
) -> Option<T> {
    let value = <T as FromCommandRequest>::from_command_request(request);
    value.ok()
}

pub fn from_command_request_anyhow<'a, T: FromCommandRequest<'a, 'a> + 'a>(
    request: &'a CommandRequest<'a>,
) -> Result<T, CommandError<anyhow::Error>> {
    let value = <T as FromCommandRequest>::from_command_request(request);
    value.map_err(|err| CommandError::RequestError(anyhow::Error::new(err)))
}
