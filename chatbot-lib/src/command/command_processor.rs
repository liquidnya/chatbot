use crate::request::CommandRequest;
use crate::response::Response;
use async_trait::async_trait;

#[async_trait]
pub trait CommandProcessor {
    async fn process<'a>(&self, request: &'a CommandRequest<'a>) -> Option<Response<'a>>;
}
