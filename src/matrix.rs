use crate::{
    event::{Event, MatrixMessageReceiveEvent},
    Cli,
};
use anyhow::Result;
use matrix_sdk::{
    room::Room,
    ruma::events::{
        room::message::{MessageEventContent, MessageType, TextMessageEventContent},
        AnyMessageEventContent, SyncMessageEvent,
    },
    Client, SyncSettings,
};
use tokio::{sync::broadcast::Sender, task::JoinHandle};

fn get_message_body(event: &SyncMessageEvent<MessageEventContent>) -> Option<&String> {
    if let SyncMessageEvent {
        content:
            MessageEventContent {
                msgtype: MessageType::Text(TextMessageEventContent { body, .. }),
                ..
            },
        ..
    } = event
    {
        Some(body)
    } else {
        None
    }
}

pub(crate) async fn login(tx: Sender<Event>, args: Cli) -> Result<Client> {
    log::info!("Logging into Matrix homeserver...");
    let client = Client::new_from_user_id(args.matrix_username.clone()).await?;
    client
        .login(
            args.matrix_username.localpart(),
            &args.matrix_password,
            None,
            Some("matrix-remote-closedown"),
        )
        .await?;

    log::info!("Performing initial sync...");
    client.sync_once(SyncSettings::default()).await?;

    log::info!("Successfully logged in to Matrix homeserver");

    client
        .register_event_handler({
            let tx = tx.clone();
            move |event: SyncMessageEvent<MessageEventContent>, room: Room| {
                let tx = tx.clone();
                async move {
                    if let Room::Joined(room) = room {
                        if let Some(msg_body) = get_message_body(&event) {
                            log::debug!(
                                "Received message \"{}\" in room {}",
                                msg_body,
                                room.room_id()
                            );
                            crate::send_event!(
                                tx,
                                Event::MatrixMessageReceive(MatrixMessageReceiveEvent {
                                    room: room.room_id().clone(),
                                    body: msg_body.to_string(),
                                    sender: event.sender,
                                })
                            );
                        }
                    }
                }
            }
        })
        .await;

    Ok(client)
}

pub(crate) fn run_send_task(tx: Sender<Event>, client: Client) -> Result<JoinHandle<()>> {
    let mut rx = tx.subscribe();

    Ok(tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                Event::Exit => {
                    log::debug!("Task exit");
                    return;
                }
                Event::MatrixMessageSend(msg) => {
                    log::debug!("Sending message...");
                    if let Err(e) = client
                        .get_joined_room(&msg.room)
                        .unwrap()
                        .send(
                            AnyMessageEventContent::RoomMessage(MessageEventContent::new(
                                MessageType::Text(TextMessageEventContent::markdown(msg.body)),
                            )),
                            None,
                        )
                        .await
                    {
                        log::error!("Failed to send message: {}", e);
                    }
                }
                _ => {}
            }
        }
    }))
}
