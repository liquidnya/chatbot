use std::borrow::Cow;

use async_trait::async_trait;
use tokio::io;

pub struct Response<'a>(Option<Cow<'a, str>>, bool, bool);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReplyResponse<T>(pub(super) T);

impl<T> From<T> for ReplyResponse<T> {
    fn from(value: T) -> Self {
        ReplyResponse(value)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CommandResponse<T>(pub(super) T);

impl<T> From<T> for CommandResponse<T> {
    fn from(value: T) -> Self {
        CommandResponse(value)
    }
}

#[async_trait]
pub trait Responder {
    async fn respond(&mut self, response: &Response<'_>) -> io::Result<()>;
}

impl<'a> Response<'a> {
    pub fn new<T: Into<Cow<'a, str>>>(response: T) -> Self {
        Self(Some(response.into()), false, false)
    }

    pub fn as_reply(self) -> Self {
        Self(self.0, true, self.2)
    }

    pub fn as_command(self) -> Self {
        Self(self.0, self.1, true)
    }

    pub fn none() -> Self {
        Self(None, false, false)
    }

    pub fn response(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub fn reply(&self) -> bool {
        self.1
    }

    pub fn command(&self) -> bool {
        self.2
    }
}
