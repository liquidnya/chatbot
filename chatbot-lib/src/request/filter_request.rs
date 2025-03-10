use super::{Bot, Channel, Sender};
use crate::{
    chat_bot::StateError,
    response::Responder,
    state::{ChannelChatters, ChannelState, ChannelStateError},
    State,
};
use std::future::Future;
use std::pin::Pin;

pub type FilterPredicate = Box<
dyn
     for<'req> FnMut(
        FilterRequest<'req>,
        &'req mut dyn Responder,
    ) -> Pin<Box<dyn Future<Output = bool> + 'req>>,
>;

#[derive(Debug, Clone)]
pub struct FilterRequest<'req> {
    message: &'req str,
    sender: Sender<'req>,
    channel: Channel<'req>,
    bot: &'req Bot<'req>,
    pub(crate) context: Option<&'req crate::chat_bot::ChatBotContext<'req>>,
}

impl<'req> FilterRequest<'req> {
    pub(crate) fn new<S: Into<Sender<'req>>, Ch: Into<Channel<'req>>>(
        message: &'req str,
        sender: S,
        channel: Ch,
        bot: &'req Bot<'req>,
        context: &'req crate::chat_bot::ChatBotContext<'req>,
    ) -> Self {
        FilterRequest {
            message,
            sender: sender.into(),
            channel: channel.into(),
            bot,
            context: Some(context),
        }
    }

    pub fn message(&self) -> &str {
        self.message
    }

    pub fn sender(&self) -> &Sender<'req> {
        &self.sender
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn bot(&self) -> &Bot {
        self.bot
    }

    pub fn chatters(&self) -> Option<ChannelChatters> {
        self.context.map(|c| c.chatters())
    }

    pub fn state<'a, T: Send + Sync + 'static>(&'a self) -> Result<State<'req, T>, StateError> {
        self.context.ok_or(StateError::NoContext)?.state()
    }

    pub fn channel_state<'a, T: Send + Sync + 'static>(
        &'a self,
    ) -> Result<ChannelState<'req, T>, ChannelStateError> {
        self.context
            .ok_or(ChannelStateError::NoContext)?
            .channel_state()
    }
}
