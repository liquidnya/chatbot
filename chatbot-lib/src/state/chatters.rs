use crate::request::Channel;
use crate::request::Sender;
use crate::user::ChannelId;
use crate::user::OwnedUser;
use crate::user::User;
use crate::user::UserArgument;
use crate::user::UserId;
use async_trait::async_trait;
use chashmap::CHashMap;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug)]
struct UserEntry {
    username: String,
    display_name: Option<String>,
    last_chatted: Instant,
    last_message: String,
    last_message_id: MessageId,
}

// TODO: FIXME: use async synchronization instead! locking on an async thread might be bad
#[derive(Debug, Clone, Default)]
pub struct ChannelChatters {
    chatters: Arc<CHashMap<ChannelId, Arc<CHashMap<UserId, UserEntry>>>>,
    channels: Arc<CHashMap<String, ChannelId>>,
    all_chatters: Arc<RwLock<AllChatters>>,
    all_channels: Arc<RwLock<AllChannels>>,
}

#[derive(Debug, Clone, Default)]
struct AllChatters {
    usernames: HashMap<String, usize>,
    display_names: HashMap<String, usize>,
    user_ids: HashMap<UserId, usize>,
    users: Vec<OwnedUser>,
}

#[derive(Debug, Clone, Default)]

struct AllChannels {
    chatters: AllChatters,
    //entries: Vec<ChannelEntry>,
}
/*
#[derive(Debug, Clone, Default)]

struct ChannelEntry {
}
*/

#[derive(Debug, Clone, PartialEq, Eq)]
enum MessageId {
    String(String),
    Uuid(Uuid),
}

impl PartialEq<str> for MessageId {
    fn eq(&self, other: &str) -> bool {
        match (self, other) {
            (Self::String(lhs), rhs) => lhs == rhs,
            (Self::Uuid(lhs), rhs) => Uuid::try_parse(rhs)
                .ok()
                .map_or_else(|| false, |rhs: Uuid| lhs == &rhs),
        }
    }
}

impl From<&str> for MessageId {
    fn from(value: &str) -> Self {
        match Uuid::try_parse(value) {
            Ok(uuid) => MessageId::Uuid(uuid),
            _ => MessageId::String(value.into()),
        }
    }
}

#[async_trait]
trait NoticeChatter {
    async fn notice_chatter(&self, chatter: &User);
}

#[async_trait]
impl NoticeChatter for Arc<RwLock<AllChatters>> {
    async fn notice_chatter(&self, chatter: &User) {
        let chatters = self.read().await;
        if chatters.needs_update_or_insert(chatter).is_none() {
            // TODO: updgrade the lock from reading to writing instead?
            drop(chatters);
            let mut chatters = self.write().await;
            chatters.update_or_insert(chatter);
        }
    }
}

#[async_trait]
impl NoticeChatter for Arc<RwLock<AllChannels>> {
    async fn notice_chatter(&self, chatter: &User) {
        let chatters = self.read().await;
        if chatters.chatters.needs_update_or_insert(chatter).is_none() {
            // TODO: updgrade the lock from reading to writing instead?
            drop(chatters);
            let mut chatters = self.write().await;
            chatters.chatters.update_or_insert(chatter);
        }
    }
}

impl AllChatters {
    fn index(&self, chatter: &User) -> Option<usize> {
        if let Some(user_id) = chatter.user_id() {
            self.user_ids.get(&user_id).cloned()
        } else {
            self.usernames.get(chatter.username()).cloned()
        }
    }

    fn index_from_userargument(&self, user: &UserArgument) -> Option<usize> {
        self.usernames
            .get(user.as_argument())
            .cloned()
            .or_else(|| self.display_names.get(user.as_argument()).cloned())
    }

    fn needs_update_or_insert(&self, chatter: &User) -> Option<usize> {
        if let Some(index) = self.index(chatter) {
            let user = &self.users[index];
            // update
            if user.username() != chatter.username()
                || user.display_name() != chatter.display_name()
            {
                None
            } else {
                Some(index)
            }
        } else {
            // insert
            None
        }
    }

    fn insert(&mut self, chatter: &User) -> usize {
        log::debug!(
            "new chatter {:?}{}",
            chatter,
            if chatter.user_id().is_none() {
                " (or username changed)"
            } else {
                ""
            }
        );

        let index = self.users.len();
        self.users.push(OwnedUser::from_user(chatter));
        self.usernames.insert(chatter.username().to_owned(), index);
        if let Some(display_name) = chatter.display_name() {
            self.display_names.insert(display_name.to_owned(), index);
        }
        if let Some(user_id) = chatter.user_id() {
            self.user_ids.insert(user_id, index);
        }
        index
    }

    fn update_or_insert(&mut self, chatter: &User) -> usize {
        if let Some(index) = self.index(chatter) {
            // update
            let user = &mut self.users[index];
            let previous_username = user.update_username(chatter.username());
            let previous_display_name = user.update_display_name(chatter.display_name());
            let insert_user_id = user.set_user_id(chatter.user_id());
            if let Some(previous_username) = previous_username {
                log::debug!(
                    "username changed from {} to {} [user id {:?}]",
                    previous_username,
                    chatter.username(),
                    chatter.user_id()
                );
                self.usernames.remove(&previous_username);
                self.usernames.insert(chatter.username().to_owned(), index);
            }
            if let Some(previous_display_name) = previous_display_name {
                log::debug!(
                    "display name changed from {:?} to {:?} [username {}, user id {:?}]",
                    previous_display_name,
                    chatter.display_name(),
                    chatter.username(),
                    chatter.user_id()
                );
                if let Some(previous_display_name) = previous_display_name {
                    self.display_names.remove(&previous_display_name);
                }
                if let Some(display_name) = chatter.display_name() {
                    self.display_names.insert(display_name.to_owned(), index);
                }
            }
            if let Some(user_id) = insert_user_id {
                log::debug!(
                    "updated user id {} of username {}",
                    user_id,
                    chatter.username()
                );
                self.user_ids.insert(user_id, index);
            }
            index
        } else {
            // insert
            self.insert(chatter)
        }
    }
}

impl ChannelChatters {
    pub fn new() -> Self {
        ChannelChatters::default()
    }

    pub async fn get<'a, 'b, T: 'a>(&self, user: T) -> Option<OwnedUser>
    where
        T: Into<UserArgument<'a>>,
    {
        let argument = user.into();

        let chatters = self.all_chatters.read().await;
        chatters
            .index_from_userargument(&argument)
            .map(|index| chatters.users[index].clone())
    }

    pub async fn clear_chat(
        &self,
        channel: &'_ Channel<'_>,
        user_id: Option<UserId>,
        name: Option<&str>,
    ) {
        fn clear(
            chatters: &ChannelChatters,
            channel_id: ChannelId,
            user_id: Option<UserId>,
            name: Option<&str>,
        ) {
            let chatters = chatters.chatters.get(&channel_id);
            if let Some(chatters) = chatters {
                if let Some(user_id) = user_id {
                    chatters.remove(&user_id);
                } else if let Some(username) = name {
                    // slow :(
                    chatters.retain(|_key, value| value.username != username);
                } else {
                    chatters.clear();
                }
            }
        }

        if let Some(channel_id) = channel.user_id() {
            clear(self, channel_id, user_id, name);
        } else if let Some(channel_id) = self.channels.get(channel.username()) {
            clear(self, *channel_id, user_id, name);
        } else {
            // fallback clear all chatters from all channels D:
            self.chatters.clear();
        }
    }

    pub async fn clear_message(
        &self,
        channel: &'_ Channel<'_>,
        message_id: Option<&'_ str>,
        login: Option<&'_ str>,
    ) {
        fn clear(
            chatters: &ChannelChatters,
            channel_id: ChannelId,
            message_id: Option<&str>,
            login: Option<&str>,
        ) {
            let chatters = chatters.chatters.get(&channel_id);
            if let Some(chatters) = chatters {
                if message_id.is_some() || login.is_some() {
                    // slow :(
                    let message_id: Option<MessageId> = message_id.map(MessageId::from);
                    chatters.retain(|_key, value| {
                        message_id
                            .as_ref()
                            .map_or(true, |message_id| &value.last_message_id != message_id)
                            && login.map_or(true, |username| value.username != username)
                    });
                } else {
                    // fallback clear all chatters D:
                    chatters.clear();
                }
            }
        }

        if let Some(channel_id) = channel.user_id() {
            clear(self, channel_id, message_id, login);
        } else if let Some(channel_id) = self.channels.get(channel.username()) {
            clear(self, *channel_id, message_id, login);
        } else {
            // fallback clear all chatters from all channels D:
            self.chatters.clear();
        }
    }

    pub async fn notice_chatter(
        &self,
        channel: &'_ Channel<'_>,
        sender: &'_ Sender<'_>,
        data: &str,
        message_id: &str,
    ) {
        self.all_chatters.notice_chatter(sender).await;
        self.all_channels.notice_chatter(channel).await;

        let user_entry = || UserEntry {
            username: sender.username().to_owned(),
            display_name: sender.display_name().map(String::from),
            last_chatted: Instant::now(),
            last_message: data.to_owned(),
            last_message_id: message_id.into(),
        };
        if let Some(channel_id) = channel.user_id() {
            if let Some(user_id) = sender.user_id() {
                let chatters = self.chatters.get(&channel_id);
                if let Some(chatters) = chatters {
                    chatters.upsert(user_id, user_entry, |user| {
                        user.last_chatted = Instant::now();
                        if user.last_message != data {
                            user.last_message = data.to_owned();
                        }
                        if &user.last_message_id != message_id {
                            user.last_message_id = message_id.into();
                        }
                        if user.username != sender.username() {
                            user.username = sender.username().to_owned();
                        }
                        if user.display_name.as_deref() != sender.display_name() {
                            user.display_name = sender.display_name().map(String::from);
                        }
                    });
                } else {
                    drop(chatters);
                    self.channels.insert(channel.username().into(), channel_id);
                    let chatters = Arc::new(CHashMap::new());
                    chatters.insert(user_id, (user_entry)());
                    self.chatters.insert(channel_id, chatters);
                }
            }
        }
        // TODO: add some cleanup to chatters maybe from time to time
        // log::error!("{:?}", self.chatters);
    }

    pub async fn get_list(
        &self,
        channel_id: ChannelId,
        from: Duration,
        display_name: bool,
    ) -> Vec<String> {
        if let Some(chatters) = self.chatters.get(&channel_id) {
            let chatters = chatters.clone();
            // read guard should be dropped here
            // TODO: this is very slow and bad code :(
            // but it is not called often, so maybe it's fine?
            let result = Arc::new(Mutex::new(vec![]));
            chatters.retain(|_, v| {
                if v.last_chatted.elapsed() < from {
                    let mut result = result.lock().unwrap();
                    if display_name {
                        if let Some(display_name) = &v.display_name {
                            result.push(display_name.clone());
                        } else {
                            result.push(v.username.clone());
                        }
                    } else {
                        result.push(v.username.clone());
                    }
                }
                true
            });
            return Arc::try_unwrap(result).unwrap().into_inner().unwrap();
        }
        vec![]
    }

    pub async fn get_random_message(
        &self,
        channel_id: ChannelId,
        from: Duration,
    ) -> Option<String> {
        if let Some(chatters) = self.chatters.get(&channel_id) {
            let chatters = chatters.clone();
            // read guard should be dropped here
            // TODO: this is very slow and bad code :(
            // but it is not called often, so maybe it's fine?
            let result = Arc::new(Mutex::new(vec![]));
            chatters.retain(|_, v| {
                if v.last_chatted.elapsed() < from {
                    let mut result = result.lock().unwrap();
                    result.push(v.last_message.clone());
                }
                true
            });
            let list = Arc::try_unwrap(result).unwrap().into_inner().unwrap();
            let mut rng = rand::thread_rng();
            return list.choose(&mut rng).map(|x| x.to_owned());
        }
        None
    }
}
