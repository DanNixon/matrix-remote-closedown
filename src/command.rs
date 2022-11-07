use anyhow::{anyhow, Error};
use prometheus_client::encoding::text::Encode;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Command {
    pub station_name: String,
    pub op: Operation,
}

impl Default for Command {
    fn default() -> Self {
        Command {
            station_name: String::default(),
            op: Operation::Help,
        }
    }
}

impl TryFrom<String> for Command {
    type Error = Error;

    fn try_from(cmd_str: String) -> Result<Self, Self::Error> {
        let cmd_str = cmd_str.to_lowercase();
        let parts: Vec<&str> = cmd_str.split(' ').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            Err(anyhow!("Cannot parse anything from an empty string"))
        } else if parts[0].starts_with('!') {
            Ok(Command {
                station_name: parts[0][1..].to_string(),
                op: parts[1..].try_into()?,
            })
        } else {
            Err(anyhow!("Failed to parse start of command"))
        }
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Encode)]
pub(crate) enum Operation {
    Help,
    Shutdown,
    PowerOn,
    PowerOff,
    PttEnable,
    PttDisable,
}

impl Operation {
    pub(crate) fn is_operator_only(&self) -> bool {
        match self {
            Self::Help => false,
            Self::Shutdown => true,
            Self::PowerOff => true,
            Self::PowerOn => true,
            Self::PttEnable => true,
            Self::PttDisable => true,
        }
    }
}

impl TryFrom<&[&str]> for Operation {
    type Error = Error;

    fn try_from(parts: &[&str]) -> Result<Self, Self::Error> {
        match parts {
            ["help"] => Ok(Self::Help),
            ["shutdown"] => Ok(Self::Shutdown),
            ["power", "on"] => Ok(Self::PowerOn),
            ["power", "off"] => Ok(Self::PowerOff),
            ["ptt", "enable"] => Ok(Self::PttEnable),
            ["ptt", "disable"] => Ok(Self::PttDisable),
            _ => Err(anyhow!("Unknown command")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_ok() {
        assert_eq!(
            Command::try_from("!mb7pmf power on".to_string()).unwrap(),
            Command {
                station_name: "mb7pmf".to_string(),
                op: Operation::PowerOn,
            }
        );
    }

    #[test]
    fn parse_command_ok_whitespace() {
        assert_eq!(
            Command::try_from(" !mb7pmf   power  on ".to_string()).unwrap(),
            Command {
                station_name: "mb7pmf".to_string(),
                op: Operation::PowerOn,
            }
        );
    }

    #[test]
    fn parse_command_ok_case() {
        assert_eq!(
            Command::try_from("!MB7PMF Power ON".to_string()).unwrap(),
            Command {
                station_name: "mb7pmf".to_string(),
                op: Operation::PowerOn,
            }
        );
    }

    #[test]
    fn parse_command_err_command_string() {
        assert!(Command::try_from("mb7pmf power on".to_string()).is_err());
    }

    #[test]
    fn parse_operation_ok() {
        assert_eq!(Operation::try_from(&["help"][..]).unwrap(), Operation::Help);
        assert_eq!(
            Operation::try_from(&["shutdown"][..]).unwrap(),
            Operation::Shutdown
        );
        assert_eq!(
            Operation::try_from(&["power", "on"][..]).unwrap(),
            Operation::PowerOn
        );
    }

    #[test]
    fn parse_operation_err() {
        assert!(Operation::try_from(&["halp"][..]).is_err());
        assert!(Operation::try_from(&[""][..]).is_err());
        assert!(Operation::try_from(&[][..]).is_err());
    }
}
