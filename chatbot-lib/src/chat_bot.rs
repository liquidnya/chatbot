use crate::command::CommandProcessor;
use crate::request::{Bot, Channel, Command, CommandRequest, FromCommandRequest, Sender};
use crate::response::Response;
use crate::state::{CachedChannelContainer, ChannelContainer, ChannelState, ChannelStateError, ChannelChatters};
use crate::user::User;
use derive_more::{Deref, From};
use fmt::Display;
use futures_io::{AsyncRead, AsyncWrite};
use state::Container;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::io::Write;
use std::sync::Arc;
use tokio_compat_02::FutureExt;
use twitchchat::Encodable;
use twitchchat::connector::Connector;
use twitchchat::messages::Commands;
use twitchchat::messages::Privmsg;
use twitchchat::runner::Identity;
use twitchchat::writer::AsyncWriter;
use twitchchat::writer::MpscWriter;
use twitchchat::AsyncRunner;
use twitchchat::Status;
use twitchchat::UserConfig;

#[derive(Debug, Clone, Deref, From)]
pub struct State<'req, T: Send + Sync + 'static>(&'req T);
#[derive(Debug)]
pub enum StateError {
    NoContext,
    NoValue(&'static str),
}

impl Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            StateError::NoContext => write!(f, "CommandRequest is missing context"),
            StateError::NoValue(type_name) => write!(f, "No value set for type {}", type_name),
        }
    }
}

impl std::error::Error for StateError {}

impl<'a, 'req, T: Send + Sync + 'static> FromCommandRequest<'a, 'req> for State<'req, T> {
    type Error = StateError;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        request
            .context
            .ok_or(StateError::NoContext)?
            .container
            .try_get()
            .ok_or_else(|| StateError::NoValue(std::any::type_name::<T>()))
            .map(State::from)
    }
}

impl<'a, 'req, T: Send + Sync + 'static> FromCommandRequest<'a, 'req> for ChannelState<'req, T> {
    type Error = ChannelStateError;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        request
            .context
            .ok_or(ChannelStateError::NoContext)?
            .channel_container
            .ok_or(ChannelStateError::NoChannelContainer)?
            .try_get()
            .ok_or_else(|| ChannelStateError::NoValue(std::any::type_name::<T>()))
            .map(ChannelState::from)
    }
}

#[derive(Clone)]
pub(crate) struct ChatBotContext<'req> {
    container: &'req Container![Send + Sync],
    channel_container: Option<&'req Container![Send + Sync]>,
}

impl<'req> ChatBotContext<'req> {
    fn new(container: &'req Container![Send + Sync], channel_container: Option<&'req Container![Send + Sync]>) -> Self {
        Self {
            container,
            channel_container,
        }
    }
}

impl<'req> std::fmt::Debug for ChatBotContext<'req> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("..")
    }
}

pub struct ChatBot<'a, C, P> {
    connector: C,
    command_processor: P,
    user_config: &'a UserConfig,
    container: Container![Send + Sync],
    channel_container: Option<&'a ChannelContainer>,
    chatters: ChannelChatters,
}

impl<'a, C> ChatBot<'a, C, ()> {
    pub fn new(connector: C, user_config: &'a UserConfig) -> Self {
        Self {
            connector,
            command_processor: (),
            user_config,
            container: <Container![Send + Sync]>::new(),
            channel_container: Option::<&'a ChannelContainer>::None,
            chatters: ChannelChatters::new(),
        }
    }

    pub fn with_command_processor<P>(self, command_processor: P) -> ChatBot<'a, C, P>
    where
        P: CommandProcessor,
    {
        ChatBot {
            connector: self.connector,
            command_processor,
            user_config: self.user_config,
            container: self.container,
            channel_container: self.channel_container,
            chatters: ChannelChatters::new(),
        }
    }
}

impl<'a, C, P> ChatBot<'a, C, P> {
    pub fn with_state<T: Sync + Send + 'static>(self, state: T) -> Self {
        self.container.set(state); // TODO: do something if the state was already set
        self
    }

    pub fn with_channel_state<'b, 'c: 'b>(
        self,
        channel_container: &'c ChannelContainer,
    ) -> ChatBot<'b, C, P>
    where
        'a: 'b,
    {
        ChatBot {
            connector: self.connector,
            command_processor: self.command_processor,
            user_config: self.user_config,
            container: self.container,
            channel_container: Some(channel_container),
            chatters: ChannelChatters::new(),
        }
    }

    pub fn chatters(&self) -> ChannelChatters{
        self.chatters.clone()
    }
}

#[derive(Debug)]
pub enum IdentityError {
    Anonymous,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Anonymous user: No username found.")
    }
}

impl Error for IdentityError {}

impl<'a> TryFrom<&'a Identity> for Bot<'a> {
    type Error = IdentityError;
    fn try_from(value: &'a Identity) -> Result<Self, Self::Error> {
        match value {
            Identity::Anonymous { .. } => Err(IdentityError::Anonymous),
            Identity::Basic { name, .. } => Ok(User::from_username(name).into()),
            Identity::Full {
                name,
                user_id,
                display_name,
                ..
            } => Ok(User::new(name, display_name.as_deref(), Some(*user_id)).into()),
        }
    }
}

impl<'a> From<&'a UserConfig> for Bot<'a> {
    fn from(value: &'a UserConfig) -> Self {
        User::from_username(&value.name).into()
    }
}

#[derive(Debug)]
pub enum PrivmsgCommandError {
    DoesNotStartWithBang,
}

impl fmt::Display for PrivmsgCommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Command does not start with `!`.")
    }
}

impl Error for PrivmsgCommandError {}

impl<'a> TryFrom<&'a Privmsg<'_>> for Command<'a> {
    type Error = PrivmsgCommandError;
    fn try_from(message: &'a Privmsg) -> Result<Self, Self::Error> {
        let data = message.data().trim_start();
        if data.starts_with('!') {
            Ok(data.into())
        } else {
            Err(PrivmsgCommandError::DoesNotStartWithBang)
        }
    }
}

impl<'a> From<&'a Privmsg<'_>> for Sender<'a> {
    fn from(value: &'a Privmsg) -> Self {
        let user_id = value.user_id().and_then(|value| value.try_into().ok()); // TODO: user_id is u64 instead of i64
        Sender::new(
            User::new(value.name(), value.display_name(), user_id),
            value.is_moderator(),
            value.is_broadcaster(),
        )
    }
}

impl<'a> From<&'a Privmsg<'_>> for Channel<'a> {
    fn from(value: &'a Privmsg) -> Self {
        let user_id = value.room_id().and_then(|value| value.try_into().ok()); // TODO: user_id is u64 instead of i64
        User::new(value.channel().trim_start_matches('#'), None, user_id).into()
    }
}

struct MessageHandler<'msg, P> {
    bot: &'msg Bot<'msg>,
    container: &'msg Container![Send + Sync],
    channel_container: Option<CachedChannelContainer<'msg>>,
    command_processor: &'msg P,
    writer: AsyncWriter<MpscWriter>,
    chatters: ChannelChatters,
}

pub struct PrivmsgReply<'a> {
    pub(crate) reply_to: &'a Privmsg<'a>,
    pub(crate) msg: &'a str,
}

macro_rules! write_nl {
    ($w:expr, $fmt:expr, $($args:expr),* $(,)?) => {{
        write!($w, $fmt, $($args),*)?;
        write!($w, "\r\n")
    }};
}

impl<'a> Encodable for PrivmsgReply<'a> {
    fn encode<W>(&self, buf: &mut W) -> std::io::Result<()>
    where
        W: Write + ?Sized,
    {
        if !self.msg.trim_start().starts_with(|c| c == '.' || c == '/') { // do not reply when using a twitch command
            if let Some(id) = self.reply_to.tags().get("id") { // find message id to reply to
                return write_nl!(buf, "@reply-parent-msg-id={} PRIVMSG {} :{}", id, twitchchat::commands::Channel::new(self.reply_to.channel()), self.msg);
            }
        }
        write_nl!(buf, "PRIVMSG {} :{}", twitchchat::commands::Channel::new(self.reply_to.channel()), self.msg)
    }
}

pub const fn privmsg_reply<'a>(reply_to: &'a Privmsg<'a>, msg: &'a str) -> PrivmsgReply<'a> {
    PrivmsgReply { reply_to, msg }
}

impl<'msg, P> MessageHandler<'msg, P>
where
    P: CommandProcessor,
{
    fn new(
        bot: &'msg Bot<'msg>,
        container: &'msg Container![Send + Sync],
        channel_container: Option<CachedChannelContainer<'msg>>,
        command_processor: &'msg P,
        writer: AsyncWriter<MpscWriter>,
        chatters: ChannelChatters,
    ) -> Self {
        Self {
            bot,
            container,
            channel_container,
            command_processor,
            writer,
            chatters,
        }
    }

    async fn handle(&mut self, message: &'_ Privmsg<'_>) -> Result<(), Box<dyn Error>> {
        let bot = self.bot;
        let container = self.container;

        let channel: Channel =message.into();
        let sender: Sender = message.into();


        self.chatters.notice_chatter(&channel, &sender, message.data());

        if let Ok(command) = Command::try_from(message) {
            // unpack channel container at the last moment possible
            let mut channel_container_rc = None;
            if let Some(channel_container) = &mut self.channel_container {
                channel_container_rc = Some(channel_container.get(message.channel()).await);
            }

            let context = ChatBotContext::new(
                container,
                channel_container_rc
                    .as_ref()
                    .map(|rc| rc as &Arc<Container![Send + Sync]> as &Container![Send + Sync]),
            );
            let request = CommandRequest::new(command, sender, channel, bot, &context);
            if request.sender() as &User == bot as &User {
                return Ok(()); // do not handle messages from the bot
            }
            if let Some(response) = self
                .command_processor
                .process(&request)
                .await
                .as_ref()
                .and_then(Response::response)
                // TODO: check if filter is necessary
                .filter(|response| !response.trim_start().starts_with('/') || response.trim_start().starts_with("/announce"))
                .filter(|response| !response.trim_start().starts_with('.') || response.trim_start().starts_with(".announce"))
            {
                let message = privmsg_reply(message, response);
                self.writer.encode(message).compat().await?;
            }
        }
        Ok(())
    }
}

impl<'a, C, P> ChatBot<'a, C, P>
where
    C: Connector,
    for<'o> &'o C::Output: AsyncRead + AsyncWrite + Send + Sync + Unpin,
    P: CommandProcessor,
{
    pub async fn run(
        self,
        channels: impl std::iter::IntoIterator<Item = &str>,
    ) -> Result<(), Box<dyn Error>> {
        let user_config = self.user_config;
        let command_processor = self.command_processor;
        let channel_container = self.channel_container;
        let mut container = self.container;

        container.freeze();
        let mut runner = AsyncRunner::connect(self.connector, user_config)
            .compat()
            .await?;
        let identity = runner.identity.clone(); // TODO: store bot user somewhere in memeory
        let bot: Bot = (&identity)
            .try_into()
            .unwrap_or_else(|_| user_config.into());

        log::info!("Connected as {}", bot.username());

        // TODO: join channels
        //runner.join(bot.username()).compat().await?;
        //log::info!("Joined channel {}", bot.username());
        for channel in channels {
            runner.join(channel).compat().await?;
            log::info!("Joined channel {}", channel);
        }

        let mut handler = MessageHandler::new(
            &bot,
            &container,
            channel_container.map(ChannelContainer::create_local_cache),
            &command_processor,
            runner.writer(),
            self.chatters.clone(),
        );

        loop {
            // TODO: add CTRL+C detection!
            let message = runner.next_message().compat().await?;
            match message {
                Status::Message(commands) => {
                    log::trace!("Message: {:#?}", commands);
                    match commands {
                        Commands::Privmsg(message) => handler.handle(&message).await?,
                        Commands::Ping(_) | Commands::Pong(_) => {}
                        _ => {}
                    }
                }
                Status::Quit | Status::Eof => break,
            }
        }
        Ok(())
    }
}
