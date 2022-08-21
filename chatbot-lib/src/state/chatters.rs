
use chashmap::CHashMap;
use std::sync::Arc;
use std::time::Instant;
use std::time::Duration;
use crate::request::Sender;
use crate::request::Channel;
use std::sync::Mutex;
use rand::seq::SliceRandom;

type ChannelId = i64;
type UserId = i64;

#[derive(Debug)]
struct UserEntry {
    username: String,
    display_name: Option<String>,
    last_chatted: Instant,
    last_message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelChatters {
    chatters: Arc<CHashMap<ChannelId, Arc<CHashMap<UserId, UserEntry>>>>,
}

impl ChannelChatters {
    pub fn new() -> Self {
        ChannelChatters {
            chatters: Arc::new(CHashMap::new()),
        }
    }

    pub fn notice_chatter(&self, channel: &Channel, sender: &Sender, data: &str) {
        let user_entry = || UserEntry {
            username: sender.username().to_owned(),
            display_name: sender.display_name().map(String::from),
            last_chatted: Instant::now(),
            last_message: data.to_owned(),
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
                        if user.username != sender.username() {
                            user.username = sender.username().to_owned();
                        }
                        if user.display_name.as_deref() != sender.display_name() {
                            user.display_name = sender.display_name().map(String::from);
                        }
                    });
                } else {
                    let chatters = Arc::new(CHashMap::new());
                    chatters.insert(user_id, (user_entry)());
                    self.chatters.insert(channel_id, chatters);
                }
            }
        }
        // TODO: add some cleanup to chatters maybe from time to time
        // log::error!("{:?}", self.chatters);
    }

    pub fn get_list(&self, channel_id: i64, from: Duration, display_name: bool) -> Vec<String> {
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

    pub fn get_random_message(&self, channel_id: i64, from: Duration) -> Option<String> {
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
            return list.choose(&mut rng).map(|x|x.to_owned());
        }
        None
    }
}

