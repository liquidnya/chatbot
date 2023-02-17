use super::{Bot, Channel, Sender};
use derive_more::{Deref, From};

#[derive(Debug, Clone)]
pub struct CommandRequest<'req> {
    command: Command<'req>,
    sender: Sender<'req>,
    channel: Channel<'req>,
    bot: &'req Bot<'req>,
    pub(crate) context: Option<&'req crate::chat_bot::ChatBotContext<'req>>,
}

impl<'req> CommandRequest<'req> {
    pub(crate) fn new<Co: Into<Command<'req>>, S: Into<Sender<'req>>, Ch: Into<Channel<'req>>>(
        command: Co,
        sender: S,
        channel: Ch,
        bot: &'req Bot<'req>,
        context: &'req crate::chat_bot::ChatBotContext<'req>,
    ) -> Self {
        CommandRequest {
            command: command.into(),
            sender: sender.into(),
            channel: channel.into(),
            bot,
            context: Some(context),
        }
    }

    pub fn from_parts<Co: Into<Command<'req>>, S: Into<Sender<'req>>, Ch: Into<Channel<'req>>>(
        command: Co,
        sender: S,
        channel: Ch,
        bot: &'req Bot<'req>,
    ) -> Self {
        CommandRequest {
            command: command.into(),
            sender: sender.into(),
            channel: channel.into(),
            bot,
            context: None,
        }
    }
}

#[derive(Debug, Clone, Deref, From)]
pub struct Command<'a>(&'a str);

impl<'req> CommandRequest<'req> {
    pub fn command(&self) -> &Command<'req> {
        &self.command
    }
    pub fn sender(&self) -> &Sender<'req> {
        &self.sender
    }
    pub fn channel(&self) -> &Channel<'req> {
        &self.channel
    }
    pub fn bot(&self) -> &Bot<'req> {
        self.bot
    }
}
