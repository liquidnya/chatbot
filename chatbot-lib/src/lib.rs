#![deny(clippy::all)]

mod chat_bot;

pub mod command;
pub mod request;
pub mod response;
pub mod state;
pub mod user;

pub use self::chat_bot::{ChatBot, State};

#[cfg(test)]
mod tests {

    // !song add <command> <url> <cooldown>
    fn song_add(_command: String, _url: String, _cooldown: String, _channel: &Channel<'_>) {}

    use std::fmt::Debug;

    use crate::command::{next_argument_dyn, CommandError};
    use crate::request::Channel;
    use crate::request::{CommandRequest, FromCommandRequest};
    use crate::user::User;

    fn call<'s, 'a: 's, 'req: 's>(
        request: &'a CommandRequest<'req>,
    ) -> Result<(), CommandError<Box<dyn Debug + 's>>> {
        let mut iter = request.command().split_whitespace();
        let command = iter.next().ok_or(CommandError::CommandMismatch)?;
        if command != "!song" {
            return Err(CommandError::CommandMismatch);
        }
        let sub_command = iter.next().ok_or(CommandError::SubcommandMismatch)?;
        if sub_command != "add" {
            return Err(CommandError::SubcommandMismatch);
        }

        let command = next_argument_dyn(iter.next(), "channel")?;
        let url = next_argument_dyn(iter.next(), "url")?;
        let cooldown = next_argument_dyn(iter.next(), "cooldown")?;
        let channel = FromCommandRequest::from_command_request_dyn(&request)
            .map_err(|e| CommandError::RequestError(e))?;

        let _result = song_add(command, url, cooldown, channel);
        Ok(())
    }

    #[test]
    fn call_song_add() {
        let bot = User::from_username("helperblock").into();
        let request = CommandRequest::from_parts(
            "!song add !furretwalk https://example.com/ 20m",
            User::from_username("liquidblock"),
            User::from_username("liquidblock"),
            &bot,
        );
        let result = call(&request);
        let result = result.map_err(|err| format!("{:?}", err.map_err(|err| format!("{:?}", err))));
        assert_eq!(result, Ok(()));
    }
}
