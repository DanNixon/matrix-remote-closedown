use crate::command::Command;
use anyhow::Error;
use matrix_sdk::ruma::{RoomId, UserId};

#[derive(Clone, Debug)]
pub(crate) enum Event {
    MatrixMessageReceive(MatrixMessageReceiveEvent),
    MatrixMessageSend(MatrixMessageSendEvent),

    MqttStatusMessageReceived(String),
    MqttSendCommandMessage(String),

    CommandReceive(CommandEvent),

    Exit,
}

#[derive(Clone, Debug)]
pub(crate) struct MatrixMessageReceiveEvent {
    pub room: RoomId,
    pub sender: UserId,
    pub body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct MatrixMessageSendEvent {
    pub room: RoomId,
    pub body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CommandEvent {
    pub room: RoomId,
    pub cmd: Command,
}

impl TryFrom<MatrixMessageReceiveEvent> for CommandEvent {
    type Error = Error;

    fn try_from(evt: MatrixMessageReceiveEvent) -> Result<Self, Self::Error> {
        Ok(CommandEvent {
            room: evt.room,
            cmd: evt.body.try_into()?,
        })
    }
}
