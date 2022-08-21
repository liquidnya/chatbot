use std::time::Duration;
use url::Url;

use chatbot_lib::request::Channel;
use chatbot_macro::command;

#[command("!song add <command> <url> <cooldown>")]
#[allow(unused)] // TODO: maybe move into the macro
fn song_add(command: &str, url: Url, cooldown: Duration, channel: &Channel<'_>) -> String {
    todo!()
}

#[test]
fn works() {
    //song_add("", "", Duration::from_secs(0));
    //let x = commands![song_add, song_add];
}
