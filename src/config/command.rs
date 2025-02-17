use serde::{de, Deserialize};
use std::str::FromStr;

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
pub enum Command {
    Exit,
    Close,
    Spawn(String),
    Workspace(usize),
    ToggleFloat,
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exit" => Ok(Command::Exit),
            "close" => Ok(Command::Close),
            "toggle_float" => Ok(Command::ToggleFloat),
            s if s.starts_with("spawn ") => Ok(Command::Spawn(s[6..].to_string())),
            s if s.starts_with("workspace") => {
                let idx = s[9..]
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| format!("Invalid workspace index: {}", &s[9..]))?;
                if idx == 0 || idx > 10 {
                    return Err("Workspace index must be between 1 and 10".to_string());
                }
                Ok(Command::Workspace(idx - 1))
            }
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
