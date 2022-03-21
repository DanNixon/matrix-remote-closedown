use chrono::{offset::Local, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Deserialize, PartialEq)]
pub(crate) struct Status {
    pub tx_power_enabled: Option<bool>,
    pub tx_power_active: Option<bool>,
    pub ptt_enabled: Option<bool>,
    pub ptt_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Response {
    pub status: Status,
    pub message: Option<String>,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Command {
    pub enable_tx_power: Option<bool>,
    pub enable_ptt: Option<bool>,
}
