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
use state::TypeMap;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use twitch_irc::login::LoginCredentials;
use twitch_irc::message::{
    ClearChatMessage, ClearMsgMessage, PrivmsgMessage, ServerMessage,
};
use twitch_irc::transport::Transport;
use twitch_irc::{ClientConfig, TwitchIRCClient};

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

impl std::fmt::Debug for ChatBotContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("..")
    }
}

pub struct ChatBot<'a, T: Transport, L: LoginCredentials, P> {
    incoming_messages: UnboundedReceiver<ServerMessage>,
    client: TwitchIRCClient<T, L>,
    command_processor: P,
    bot_login: &'a str,
    container: TypeMap![Send + Sync],
    channel_container: Option<&'a ChannelContainer>,
    chatters: ChannelChatters,
    ignore_self: bool,
    filter: Option<FilterPredicate>,
}

impl<'a, T: Transport, L: LoginCredentials> ChatBot<'a, T, L, ()> {
    pub fn new(bot_login: &'a str, config: ClientConfig<L>) -> Self {
        let (incoming_messages, client) = TwitchIRCClient::<T, L>::new(config);
        Self {
            incoming_messages,
            client,
            command_processor: (),
            bot_login,
            container: <TypeMap![Send + Sync]>::new(),
            channel_container: Option::<&'a ChannelContainer>::None,
            chatters: ChannelChatters::new(),
            ignore_self: true,
            filter: None,
        }
    }

    pub fn with_command_processor<P>(self, command_processor: P) -> ChatBot<'a, T, L, P>
    where
        P: CommandProcessor,
    {
        ChatBot {
            incoming_messages: self.incoming_messages,
            client: self.client,
            command_processor,
            bot_login: self.bot_login,
            container: self.container,
            channel_container: self.channel_container,
            chatters: self.chatters,
            ignore_self: self.ignore_self,
            filter: self.filter,
        }
    }
}

impl<'a, T: Transport, L: LoginCredentials, P> ChatBot<'a, T, L, P> {
    pub fn with_state<S: Sync + Send + 'static>(self, state: S) -> Self {
        self.container.set(state); // TODO: do something if the state was already set
        self
    }

    pub fn with_channel_state<'b, 'c: 'b>(
        self,
        channel_container: &'c ChannelContainer,
    ) -> ChatBot<'b, T, L, P>
    where
        'a: 'b,
    {
        ChatBot {
            incoming_messages: self.incoming_messages,
            client: self.client,
            command_processor: self.command_processor,
            bot_login: self.bot_login,
            container: self.container,
            channel_container: Some(channel_container),
            chatters: self.chatters,
            ignore_self: self.ignore_self,
            filter: self.filter,
        }
    }

    pub fn process_self<'b, 'c: 'b>(self) -> ChatBot<'b, T, L, P>
    where
        'a: 'b,
    {
        ChatBot {
            incoming_messages: self.incoming_messages,
            client: self.client,
            command_processor: self.command_processor,
            bot_login: self.bot_login,
            container: self.container,
            channel_container: self.channel_container,
            chatters: self.chatters,
            ignore_self: false,
            filter: self.filter,
        }
    }

    pub fn filter<'b, 'c: 'b>(self, predicate: FilterPredicate) -> ChatBot<'b, T, L, P>
    where
        'a: 'b,
    {
        ChatBot {
            incoming_messages: self.incoming_messages,
            client: self.client,
            command_processor: self.command_processor,
            bot_login: self.bot_login,
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

impl<'a> From<&'a str> for Bot<'a> {
    fn from(value: &'a str) -> Self {
        User::from_username(value).into()
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

impl<'a> TryFrom<&'a PrivmsgMessage> for Command<'a> {
    type Error = PrivmsgCommandError;
    fn try_from(message: &'a PrivmsgMessage) -> Result<Self, Self::Error> {
        let data = message.message_text.trim_start();
        if data.starts_with('!') {
            Ok(data.into())
        } else {
            Err(PrivmsgCommandError::DoesNotStartWithBang)
        }
    }
}

impl<'a> From<&'a PrivmsgMessage> for Sender<'a> {
    fn from(value: &'a PrivmsgMessage) -> Self {
        let user_id = Some(value.sender.id.clone());
        Sender::new(
            User::new(&value.sender.login, Some(&value.sender.name), user_id),
            value.badges.iter().any(|badge| badge.name == "moderator"),
            value.badges.iter().any(|badge| badge.name == "broadcaster"),
        )
    }
}

impl<'a> From<&'a PrivmsgMessage> for Channel<'a> {
    fn from(value: &'a PrivmsgMessage) -> Self {
        let user_id = Some(value.channel_id.clone());
        // FIXME: removing '#' is probably not neccessary
        User::new(value.channel_login.trim_start_matches('#'), None, user_id).into()
    }
}

impl<'a> From<&'a ClearChatMessage> for Channel<'a> {
    fn from(value: &'a ClearChatMessage) -> Self {
        let user_id = Some(value.channel_id.clone());
        // FIXME: removing '#' is probably not neccessary
        User::new(value.channel_login.trim_start_matches('#'), None, user_id).into()
    }
}

impl<'a> From<&'a ClearMsgMessage> for Channel<'a> {
    fn from(value: &'a ClearMsgMessage) -> Self {
        // FIXME: test this
        let user_id = value.source.tags.0.get("room-id").cloned().flatten();
        // FIXME: removing '#' is probably not neccessary
        User::new(value.channel_login.trim_start_matches('#'), None, user_id).into()
    }
}

struct MessageHandler<'msg, T: Transport, L: LoginCredentials, P> {
    bot: &'msg Bot<'msg>,
    containers: Containers<'msg>,
    command_processor: &'msg P,
    client: TwitchIRCClient<T, L>,
    chatters: ChannelChatters,
    ignore_self: bool,
    filter: Option<FilterPredicate>,
}

struct MessageResponder<'a, T: Transport, L: LoginCredentials> {
    message: &'a PrivmsgMessage,
    client: TwitchIRCClient<T, L>,
}

#[async_trait]
impl<T: Transport, L: LoginCredentials> Responder for MessageResponder<'_, T, L> {
    async fn respond(&mut self, response: &crate::response::Response<'_>) -> anyhow::Result<()> {
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
                self.client
                    .say_in_reply_to(self.message, text.to_string())
                    .await?;
            } else {
                self.client
                    .say(self.message.channel_login.clone(), text.to_string())
                    .await?;
            }
        }
        Ok(())
    }
}

struct Containers<'msg> {
    container: &'msg TypeMap![Send + Sync],
    channel_container: Option<CachedChannelContainer<'msg>>,
}

impl<'msg, T: Transport, L: LoginCredentials, P> MessageHandler<'msg, T, L, P>
where
    P: CommandProcessor,
{
    fn new(
        bot: &'msg Bot<'msg>,
        containers: Containers<'msg>,
        command_processor: &'msg P,
        client: TwitchIRCClient<T, L>,
        chatters: ChannelChatters,
        ignore_self: bool,
        filter: Option<FilterPredicate>,
    ) -> Self {
        Self {
            bot,
            containers,
            command_processor,
            client,
            chatters,
            ignore_self,
            filter,
        }
    }

    async fn clear_chat(&mut self, message: &'_ ClearChatMessage) -> Result<(), Box<dyn Error>> {
        let channel: Channel = message.into();

        match &message.action {
            twitch_irc::message::ClearChatAction::ChatCleared => {
                self.chatters.clear_chat(&channel, None, None).await
            }
            twitch_irc::message::ClearChatAction::UserBanned {
                user_login,
                user_id,
            } => {
                self.chatters
                    .clear_chat(&channel, Some(user_id.clone()), Some(user_login))
                    .await
            }
            twitch_irc::message::ClearChatAction::UserTimedOut {
                user_login,
                user_id,
                ..
            } => {
                self.chatters
                    .clear_chat(&channel, Some(user_id.clone()), Some(user_login))
                    .await
            }
        }
        Ok(())
    }

    async fn clear_msg(&mut self, message: &'_ ClearMsgMessage) -> Result<(), Box<dyn Error>> {
        let channel: Channel = message.into();
        self.chatters
            .clear_message(
                &channel,
                Some(&message.message_id),
                Some(&message.sender_login),
            )
            .await;
        Ok(())
    }

    async fn handle(&mut self, message: &'_ PrivmsgMessage) -> Result<(), Box<dyn Error>> {
        let bot = self.bot;
        let container = self.containers.container;

        let channel: Channel = message.into();
        let sender: Sender = message.into();

        self.chatters
            .notice_chatter(&channel, &sender, &message.message_text, "id")
            .await;

        let mut responder = MessageResponder {
            message,
            client: self.client.clone(),
        };

        if let Some(msg_id) = Some(&message.message_id) {
            if let Some(filter) = self.filter.as_mut() {
                // TODO: create context only once
                let channel: Channel = message.into();
                let sender: Sender = message.into();
                let mut channel_container_rc = None;
                if let Some(channel_container) = &mut self.containers.channel_container {
                    channel_container_rc =
                        Some(channel_container.get(&message.channel_login).await);
                }
                let context = ChatBotContext::new(
                    container,
                    channel_container_rc
                        .as_ref()
                        .map(|rc| rc as &Arc<TypeMap![Send + Sync]> as &TypeMap![Send + Sync]),
                    &self.chatters,
                );
                let filter_request =
                    FilterRequest::new(&message.message_text, sender, channel, bot, &context);
                if !(filter)(filter_request, &mut responder).await {
                    self.chatters
                        .clear_message(&message.into(), Some(msg_id), Some(&message.sender.login))
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
                channel_container_rc = Some(channel_container.get(&message.channel_login).await);
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

impl<T: Transport, L: LoginCredentials, P> ChatBot<'_, T, L, P>
where
    P: CommandProcessor,
{
    #[allow(clippy::needless_late_init)]
    pub async fn run(
        self,
        channels: impl std::iter::IntoIterator<Item = &str>,
    ) -> Result<(), Box<dyn Error>> {
        let command_processor = self.command_processor;
        let channel_container = self.channel_container;
        let bot: Bot;
        let mut container = self.container;
        let mut handler;

        container.freeze();
        bot = self.bot_login.into();

        log::info!("Connected as {}", bot.username());

        let containers = Containers {
            container: &container,
            channel_container: channel_container.map(ChannelContainer::create_local_cache),
        };

        handler = MessageHandler::new(
            &bot,
            containers,
            &command_processor,
            self.client.clone(),
            self.chatters.clone(),
            self.ignore_self,
            self.filter,
        );
        let incoming_messages = self.incoming_messages;

        // join channels
        self.client
            .set_wanted_channels(channels.into_iter().map(|x| x.to_string()).collect())?;

            let mut incoming_messages = incoming_messages;
            while let Some(message) = incoming_messages.recv().await {
                log::trace!("Message: {:#?}", message);
                match message {
                    ServerMessage::ClearChat(message) => 
                        if handler.clear_chat(&message).await.is_err() {
                            break;
                        }
                    
                    ServerMessage::ClearMsg(message) => if handler.clear_msg(&message).await.is_err() {
                        break;
                    }
                    ServerMessage::Privmsg(message) => if handler.handle(&message).await.is_err() {
                        break;
                    }
                    _ => {}
                }
            }

        Ok(())
    }
}
