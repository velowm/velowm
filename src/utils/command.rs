use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize, Clone)]
pub enum Command {
    Exit,
    Spawn(String),
    Close,
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exit" => Ok(Command::Exit),
            "close" => Ok(Command::Close),
            cmd => Ok(Command::Spawn(cmd.to_string())),
        }
    }
}
