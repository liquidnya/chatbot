use crate::command::CommandProcessor;
use crate::request::{
    Bot, Channel, Command, CommandRequest, FilterPredicate, FilterRequest, FromCommandRequest,
    Sender,
};
use crate::response::Responder;
use crate::state::{
    CachedChannelContainer, ChannelChatters, ChannelContainer, ChannelState, ChannelStateError,
};
use crate::user::User;
use async_trait::async_trait;
use derive_more::{Deref, From};
use fmt::Display;
use futures_io::{AsyncRead, AsyncWrite};
use state::TypeMap;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::error::Error;
use std::fmt;
use std::io::Write;
use std::sync::Arc;
use tokio_compat_02::FutureExt;
use twitchchat::commands::privmsg;
use twitchchat::connector::Connector;
use twitchchat::messages::{ClearChat, Commands};
use twitchchat::messages::{ClearMsg, Privmsg};
use twitchchat::runner::Identity;
use twitchchat::writer::AsyncWriter;
use twitchchat::writer::MpscWriter;
use twitchchat::AsyncRunner;
use twitchchat::Encodable;
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
            StateError::NoValue(type_name) => write!(f, "No value set for type {type_name}"),
        }
    }
}

impl std::error::Error for StateError {}

impl<'a, 'req, T: Send + Sync + 'static> FromCommandRequest<'a, 'req> for State<'req, T> {
    type Error = StateError;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        request.context.ok_or(StateError::NoContext)?.state()
    }
}

impl<'a, 'req, T: Send + Sync + 'static> FromCommandRequest<'a, 'req> for ChannelState<'req, T> {
    type Error = ChannelStateError;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        request
            .context
            .ok_or(ChannelStateError::NoContext)?
            .channel_state()
    }
}

#[derive(Clone)]
pub(crate) struct ChatBotContext<'req> {
    container: &'req TypeMap![Send + Sync],
    channel_container: Option<&'req TypeMap![Send + Sync]>,
    chatters: &'req ChannelChatters,
}

impl<'req> ChatBotContext<'req> {
    fn new(
        container: &'req TypeMap![Send + Sync],
        channel_container: Option<&'req TypeMap![Send + Sync]>,
        chatters: &'req ChannelChatters,
    ) -> Self {
        Self {
            container,
            channel_container,
            chatters,
        }
    }

    pub fn chatters(&self) -> ChannelChatters {
        self.chatters.clone()
    }

    pub fn state<T: Send + Sync + 'static>(&self) -> Result<State<'req, T>, StateError> {
        self.container
            .try_get()
            .ok_or_else(|| StateError::NoValue(std::any::type_name::<T>()))
            .map(State::from)
    }

    pub fn channel_state<T: Send + Sync + 'static>(
        &self,
    ) -> Result<ChannelState<'req, T>, ChannelStateError> {
        self.channel_container
            .ok_or(ChannelStateError::NoChannelContainer)?
            .try_get()
            .ok_or_else(|| ChannelStateError::NoValue(std::any::type_name::<T>()))
            .map(ChannelState::from)
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
    container: TypeMap![Send + Sync],
    channel_container: Option<&'a ChannelContainer>,
    chatters: ChannelChatters,
    ignore_self: bool,
    filter: Option<FilterPredicate>,
}

impl<'a, C> ChatBot<'a, C, ()> {
    pub fn new(connector: C, user_config: &'a UserConfig) -> Self {
        Self {
            connector,
            command_processor: (),
            user_config,
            container: <TypeMap![Send + Sync]>::new(),
            channel_container: Option::<&'a ChannelContainer>::None,
            chatters: ChannelChatters::new(),
            ignore_self: true,
            filter: None,
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
            chatters: self.chatters,
            ignore_self: self.ignore_self,
            filter: self.filter,
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
            chatters: self.chatters,
            ignore_self: self.ignore_self,
            filter: self.filter,
        }
    }

    pub fn process_self<'b, 'c: 'b>(self) -> ChatBot<'b, C, P>
    where
        'a: 'b,
    {
        ChatBot {
            connector: self.connector,
            command_processor: self.command_processor,
            user_config: self.user_config,
            container: self.container,
            channel_container: self.channel_container,
            chatters: self.chatters,
            ignore_self: false,
            filter: self.filter,
        }
    }

    pub fn filter<'b, 'c: 'b>(self, predicate: FilterPredicate) -> ChatBot<'b, C, P>
    where
        'a: 'b,
    {
        ChatBot {
            connector: self.connector,
            command_processor: self.command_processor,
            user_config: self.user_config,
            container: self.container,
            channel_container: self.channel_container,
            chatters: self.chatters,
            ignore_self: self.ignore_self,
            filter: Some(predicate),
        }
    }

    pub fn chatters(&self) -> ChannelChatters {
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

impl<'a> From<&'a ClearChat<'_>> for Channel<'a> {
    fn from(value: &'a ClearChat) -> Self {
        let user_id = value.room_id().and_then(|value| value.parse().ok()); // TODO: user_id is u64 instead of i64
        User::new(value.channel().trim_start_matches('#'), None, user_id).into()
    }
}

impl<'a> From<&'a ClearMsg<'_>> for Channel<'a> {
    fn from(value: &'a ClearMsg) -> Self {
        let user_id = value.tags().get_parsed("room-id"); // TODO: user_id is u64 instead of i64
        User::new(value.channel().trim_start_matches('#'), None, user_id).into()
    }
}

struct MessageHandler<'msg, P> {
    bot: &'msg Bot<'msg>,
    containers: Containers<'msg>,
    command_processor: &'msg P,
    writer: AsyncWriter<MpscWriter>,
    chatters: ChannelChatters,
    ignore_self: bool,
    filter: Option<FilterPredicate>,
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
        log::trace!("reply message");
        if !self.msg.trim_start().starts_with(|c| c == '.' || c == '/') {
            // do not reply when using a twitch command
            if let Some(id) = self.reply_to.tags().get("id") {
                // find message id to reply to
                return write_nl!(
                    buf,
                    "@reply-parent-msg-id={} PRIVMSG {} :{}",
                    id,
                    twitchchat::commands::Channel::new(self.reply_to.channel()),
                    self.msg
                );
            }
        }
        write_nl!(
            buf,
            "PRIVMSG {} :{}",
            twitchchat::commands::Channel::new(self.reply_to.channel()),
            self.msg
        )
    }
}

pub const fn privmsg_reply<'a>(reply_to: &'a Privmsg<'a>, msg: &'a str) -> PrivmsgReply<'a> {
    PrivmsgReply { reply_to, msg }
}

struct MessageResponder<'a> {
    message: &'a Privmsg<'a>,
    writer: &'a mut AsyncWriter<MpscWriter>,
}

#[async_trait]
impl<'a> Responder for MessageResponder<'a> {
    async fn respond(&mut self, response: &crate::response::Response<'_>) -> tokio::io::Result<()> {
        if let Some(text) = response
            .response()
            // TODO: check if filter is necessary
            .filter(|response_text| {
                response.command() || !response_text.trim_start().starts_with('/')
            })
            .filter(|response_text| {
                response.command() || !response_text.trim_start().starts_with('.')
            })
            .filter(|response_text| !response_text.is_empty() && !response_text.trim().is_empty())
        {
            if response.reply() {
                let message = privmsg_reply(self.message, text);
                self.writer.encode(message).compat().await?;
            } else {
                let message = privmsg(self.message.channel(), text);
                self.writer.encode(message).compat().await?;
            }
        }
        Ok(())
    }
}

struct Containers<'msg> {
    container: &'msg TypeMap![Send + Sync],
    channel_container: Option<CachedChannelContainer<'msg>>,
}

impl<'msg, P> MessageHandler<'msg, P>
where
    P: CommandProcessor,
{
    fn new(
        bot: &'msg Bot<'msg>,
        containers: Containers<'msg>,
        command_processor: &'msg P,
        writer: AsyncWriter<MpscWriter>,
        chatters: ChannelChatters,
        ignore_self: bool,
        filter: Option<FilterPredicate>,
    ) -> Self {
        Self {
            bot,
            containers,
            command_processor,
            writer,
            chatters,
            ignore_self,
            filter,
        }
    }

    async fn clear_chat(&mut self, message: &'_ ClearChat<'_>) -> Result<(), Box<dyn Error>> {
        let channel: Channel = message.into();
        self.chatters
            .clear_chat(
                &channel,
                message.tags().get_parsed("target-user-id"),
                message.name(),
            )
            .await;
        Ok(())
    }

    async fn clear_msg(&mut self, message: &'_ ClearMsg<'_>) -> Result<(), Box<dyn Error>> {
        let channel: Channel = message.into();
        self.chatters
            .clear_message(&channel, message.target_msg_id(), message.login())
            .await;
        Ok(())
    }

    async fn handle(&mut self, message: &'_ Privmsg<'_>) -> Result<(), Box<dyn Error>> {
        let bot = self.bot;
        let container = self.containers.container;

        let channel: Channel = message.into();
        let sender: Sender = message.into();

        self.chatters
            .notice_chatter(&channel, &sender, message.data(), "id")
            .await;

        let mut responder = MessageResponder {
            message,
            writer: &mut self.writer,
        };

        if let Some(msg_id) = message.tags().get("id") {
            if let Some(filter) = self.filter.as_mut() {
                // TODO: create context only once
                let channel: Channel = message.into();
                let sender: Sender = message.into();
                let mut channel_container_rc = None;
                if let Some(channel_container) = &mut self.containers.channel_container {
                    channel_container_rc = Some(channel_container.get(message.channel()).await);
                }
                let context = ChatBotContext::new(
                    container,
                    channel_container_rc
                        .as_ref()
                        .map(|rc| rc as &Arc<TypeMap![Send + Sync]> as &TypeMap![Send + Sync]),
                    &self.chatters,
                );
                let filter_request =
                    FilterRequest::new(message.data(), sender, channel, bot, &context);
                if !(filter)(filter_request, &mut responder).await {
                    self.chatters
                        .clear_message(&message.into(), Some(msg_id), Some(message.name()))
                        .await;
                    responder
                        .respond(
                            &crate::response::Response::new(format!(".delete {msg_id}"))
                                .as_command(),
                        )
                        .await?;
                    return Ok(());
                }
            }
        }

        if let Ok(command) = Command::try_from(message) {
            log::trace!("Command found");

            // unpack channel container at the last moment possible
            let mut channel_container_rc = None;
            if let Some(channel_container) = &mut self.containers.channel_container {
                channel_container_rc = Some(channel_container.get(message.channel()).await);
            }

            let context = ChatBotContext::new(
                container,
                channel_container_rc
                    .as_ref()
                    .map(|rc| rc as &Arc<TypeMap![Send + Sync]> as &TypeMap![Send + Sync]),
                &self.chatters,
            );
            let request = CommandRequest::new(command, sender, channel, bot, &context);

            log::trace!("request: {:?}", request);

            if self.ignore_self && request.sender() as &User == bot as &User {
                log::debug!("Ignoring message from bot {:?}", bot);
                return Ok(()); // do not handle messages from the bot
            }
            if let Some(response) = self.command_processor.process(&request).await.as_ref() {
                responder.respond(response).await?;
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
    #[allow(clippy::needless_late_init)]
    pub async fn run(
        self,
        channels: impl std::iter::IntoIterator<Item = &str>,
    ) -> Result<(), Box<dyn Error>> {
        let user_config = self.user_config;
        let command_processor = self.command_processor;
        let channel_container = self.channel_container;
        let bot: Bot;
        let mut container = self.container;
        let mut runner;
        let mut handler;

        container.freeze();
        runner = AsyncRunner::connect(self.connector, user_config)
            .compat()
            .await?;
        let identity = runner.identity.clone(); // TODO: store bot user somewhere in memeory
        bot = (&identity)
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

        let containers = Containers {
            container: &container,
            channel_container: channel_container.map(ChannelContainer::create_local_cache),
        };

        handler = MessageHandler::new(
            &bot,
            containers,
            &command_processor,
            runner.writer(),
            self.chatters.clone(),
            self.ignore_self,
            self.filter,
        );

        loop {
            // TODO: add CTRL+C detection!
            let message = runner.next_message().compat().await?;
            match message {
                Status::Message(commands) => {
                    log::trace!("Message: {:#?}", commands);
                    match commands {
                        Commands::Privmsg(message) => handler.handle(&message).await?,
                        Commands::ClearChat(message) => handler.clear_chat(&message).await?,
                        Commands::ClearMsg(message) => handler.clear_msg(&message).await?,
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
