mod user_argument;

pub use self::user_argument::UserArgument;
use std::mem;

// FIXME: this should be &'a str
pub type UserId = String;
pub type ChannelId = UserId;

#[derive(Debug, Clone)]
pub struct User<'a> {
    pub(crate) username: &'a str,
    display_name: Option<&'a str>,
    pub(crate) user_id: Option<UserId>,
}

#[derive(Debug, Clone)]
pub struct OwnedUser {
    username: String,
    display_name: Option<String>,
    user_id: Option<UserId>,
}

impl<'a> User<'a> {
    pub fn new(username: &'a str, display_name: Option<&'a str>, user_id: Option<UserId>) -> Self {
        Self {
            username,
            display_name,
            user_id,
        }
    }

    pub fn from_username(username: &'a str) -> Self {
        Self {
            username,
            display_name: None,
            user_id: None,
        }
    }

    pub fn from_owned(owned: &'a OwnedUser) -> Self {
        Self {
            username: owned.username(),
            display_name: owned.display_name(),
            user_id: owned.user_id(),
        }
    }

    pub fn username(&self) -> &'a str {
        self.username
    }

    pub fn display_name(&self) -> Option<&'a str> {
        self.display_name
    }

    pub fn user_id(&self) -> Option<UserId> {
        self.user_id.clone()
    }
}

impl PartialEq for User<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.user_id, &other.user_id) {
            (Some(a), Some(b)) => a == b,
            _ => self.username == other.username,
        }
    }
}

impl OwnedUser {
    pub fn new(username: String, display_name: Option<String>, user_id: Option<UserId>) -> Self {
        Self {
            username,
            display_name,
            user_id,
        }
    }

    pub fn from_username(username: String) -> Self {
        Self {
            username,
            display_name: None,
            user_id: None,
        }
    }

    pub fn from_user(user: &User<'_>) -> Self {
        Self {
            username: user.username().to_owned(),
            display_name: user.display_name().map(String::from),
            user_id: user.user_id(),
        }
    }

    pub fn update_username(&mut self, username: &str) -> Option<String> {
        if self.username != username {
            Some(mem::replace(&mut self.username, username.to_owned()))
        } else {
            None
        }
    }

    pub fn update_display_name(&mut self, display_name: Option<&str>) -> Option<Option<String>> {
        if self.display_name.as_deref() != display_name {
            Some(mem::replace(
                &mut self.display_name,
                display_name.map(String::from),
            ))
        } else {
            None
        }
    }

    pub fn set_user_id(&mut self, user_id: Option<UserId>) -> Option<UserId> {
        if self.user_id.is_none() && user_id.is_some() {
            self.user_id = user_id;
            self.user_id.clone()
        } else {
            None
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn user_id(&self) -> Option<UserId> {
        self.user_id.clone()
    }
}

impl PartialEq for OwnedUser {
    fn eq(&self, other: &Self) -> bool {
        match (&self.user_id, &other.user_id) {
            (Some(a), Some(b)) => a == b,
            _ => self.username == other.username,
        }
    }
}
