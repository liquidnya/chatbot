use std::borrow::Cow;

pub struct Response<'a>(Option<Cow<'a, str>>);

impl<'a> Response<'a> {
    pub fn new<T: Into<Cow<'a, str>>>(response: T) -> Self {
        Self(Some(response.into()))
    }

    pub fn none() -> Self {
        Self(None)
    }

    pub fn response(&self) -> Option<&str> {
        self.0.as_deref()
    }
}
