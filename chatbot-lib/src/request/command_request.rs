use crate::user::User;
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

#[derive(Debug, Clone)]
pub struct Sender<'a> {
    user: User<'a>,
    moderator: bool,
    broadcaster: bool,
}

impl<'a> Sender<'a> {
    pub fn new(user: User<'a>, moderator: bool, broadcaster: bool) -> Self {
        Self {
            user,
            moderator,
            broadcaster,
        }
    }

    pub fn is_moderator(&self) -> bool {
        self.moderator
    }

    pub fn is_broadcaster(&self) -> bool {
        self.broadcaster
    }
}

impl<'a> From<User<'a>> for Sender<'a> {
    fn from(user: User<'a>) -> Self {
        Sender::new(user, false, false)
    }
}

impl<'a> std::ops::Deref for Sender<'a> {
    type Target = User<'a>;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

#[derive(Debug, Clone, Deref, From)]
pub struct Channel<'a>(User<'a>);
#[derive(Debug, Clone, Deref, From)]
pub struct Bot<'a>(User<'a>);
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
