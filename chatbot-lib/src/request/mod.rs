use crate::user::User;
use derive_more::{Deref, From};

mod command_request;
mod filter_request;
mod from_command_request;

#[derive(Debug, Clone, Deref, From)]
pub struct Channel<'a>(pub(crate) User<'a>);
#[derive(Debug, Clone, Deref, From)]
pub struct Bot<'a>(User<'a>);

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

pub use self::command_request::{Command, CommandRequest};
pub use self::filter_request::{FilterPredicate, FilterRequest};
pub use self::from_command_request::FromCommandRequest;
