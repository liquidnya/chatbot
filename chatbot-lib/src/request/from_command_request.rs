use crate::state::ChannelChatters;

use super::{Bot, Channel, Command, CommandRequest, Sender};
use core::fmt::Debug;

pub trait FromCommandRequest<'a, 'req>: Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error>;

    fn from_command_request_dyn<'s>(
        request: &'a CommandRequest<'req>,
    ) -> Result<Self, Box<dyn Debug + 's>>
    where
        Self: 's,
        'a: 's,
        'req: 's,
    {
        let value = <Self as FromCommandRequest>::from_command_request(request);
        value.map_err(|err| -> Box<dyn Debug + 's> { Box::new(err) })
    }
}

impl<'a, 'req> FromCommandRequest<'a, 'req> for &'a CommandRequest<'req> {
    type Error = core::convert::Infallible;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        Ok(request)
    }
}

impl<'a, 'req, T: FromCommandRequest<'a, 'req>> FromCommandRequest<'a, 'req> for Option<T> {
    type Error = core::convert::Infallible;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        Ok(<T as FromCommandRequest>::from_command_request(request).ok())
    }
}

impl<'a, 'req, T: FromCommandRequest<'a, 'req>, E> FromCommandRequest<'a, 'req> for Result<T, E>
where
    T::Error: Into<E>,
{
    type Error = core::convert::Infallible;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        Ok(<T as FromCommandRequest>::from_command_request(request).map_err(|e| e.into()))
    }
}

macro_rules! impl_from_command_request {
    ($(impl<$a:lifetime, $req:lifetime> |$request:ident| -> $ty:ty $value:block) +) => {
        $(
            impl<$a, $req> FromCommandRequest<$a, $req> for $ty {
                type Error = core::convert::Infallible;

                fn from_command_request($request: &$a CommandRequest<$req>) -> Result<Self, Self::Error> {
                    Ok($value)
                }
            }
        )+
    };
}

impl_from_command_request! {
    impl<'a, 'req> |request| -> &'a Sender<'req> { request.sender() }
    impl<'a, 'req> |request| -> &'a Channel<'req> { request.channel() }
    impl<'a, 'req> |request| -> &'a Bot<'req> { request.bot() }
    impl<'a, 'req> |request| -> &'a Command<'req> { request.command() }
    impl<'a, 'req> |request| -> Sender<'req> { request.sender().clone() }
    impl<'a, 'req> |request| -> Channel<'req> { request.channel().clone() }
    impl<'a, 'req> |request| -> Bot<'req> { request.bot().clone() }
    impl<'a, 'req> |request| -> Command<'req> { request.command().clone() }

    impl<'a, 'req> |request| -> ChannelChatters { request.context.map(|c| c.chatters()).unwrap_or_default() }
}
