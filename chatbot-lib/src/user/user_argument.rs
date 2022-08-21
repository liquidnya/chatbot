use super::User;
use crate::command::FromArgument;
use core::fmt::{Display, Error, Formatter};

#[derive(Debug, Clone)]
pub struct UserArgument<'a>(&'a str);

impl<'a> UserArgument<'a> {
    pub fn new(string: &'a str) -> Self {
        Self(string.strip_prefix('@').unwrap_or(string))
    }

    pub fn from_username(username: &'a str) -> Self {
        Self(username)
    }

    pub fn from_display_name(display_name: &'a str) -> Self {
        Self(display_name)
    }
}

impl<'a> Display for UserArgument<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        if f.alternate() {
            write!(f, "{}", self.0)
        } else {
            write!(f, "@{}", self.0)
        }
    }
}

impl<'a> PartialEq<User<'_>> for UserArgument<'a> {
    fn eq(&self, other: &User<'_>) -> bool {
        self.0 == other.username()
            || other
                .display_name()
                .map_or(false, |display_name| self.0 == display_name)
        // TODO: this is expensive and maybe not even wanted
        // || self.0.to_ascii_lowercase() == other.username()
    }
}

impl<'a> From<&User<'a>> for UserArgument<'a> {
    fn from(user: &User<'a>) -> Self {
        user.display_name()
            .map(Self::from_display_name)
            .unwrap_or_else(|| Self::from_username(user.username()))
    }
}

impl<'a> FromArgument<'a> for UserArgument<'a> {
    type Error = core::convert::Infallible;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error> {
        Ok(Self::new(argument))
    }
}
