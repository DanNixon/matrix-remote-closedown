use crate::command::Command;
use anyhow::Error;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};

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
    pub room: OwnedRoomId,
    pub sender: OwnedUserId,
    pub body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct MatrixMessageSendEvent {
    pub room: OwnedRoomId,
    pub body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CommandEvent {
    pub room: OwnedRoomId,
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
