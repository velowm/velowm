use serde::{de, Deserialize};
use std::str::FromStr;

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
pub enum Command {
    Exit,
    Close,
    Spawn(String),
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exit" => Ok(Command::Exit),
            "close" => Ok(Command::Close),
            s if s.starts_with("spawn ") => Ok(Command::Spawn(s[6..].to_string())),
            _ => Err(format!("Unknown command: {}", s)),
        }
    }
}

impl TryFrom<String> for Command {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Command::from_str(&s)
    }
}

pub fn deserialize_command<'de, D>(deserializer: D) -> Result<Command, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Command::from_str(&s).map_err(de::Error::custom)
}
