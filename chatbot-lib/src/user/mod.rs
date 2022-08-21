mod user_argument;

pub use self::user_argument::UserArgument;

#[derive(Debug, Clone)]
pub struct User<'a> {
    username: &'a str,
    display_name: Option<&'a str>,
    user_id: Option<i64>,
}

impl<'a> User<'a> {
    pub fn new(username: &'a str, display_name: Option<&'a str>, user_id: Option<i64>) -> User<'a> {
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

    pub fn username(&self) -> &'a str {
        self.username
    }

    pub fn display_name(&self) -> Option<&'a str> {
        self.display_name
    }

    pub fn user_id(&self) -> Option<i64> {
        self.user_id
    }
}

impl<'a> PartialEq for User<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self.user_id, other.user_id) {
            (Some(a), Some(b)) => a == b,
            _ => self.username == other.username,
        }
    }
}
