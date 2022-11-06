use crate::{
    event::{Event, MatrixMessageReceiveEvent},
    Cli,
};
use anyhow::Result;
use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::events::room::message::{
        MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent, TextMessageEventContent,
    },
    Client,
};
use tokio::{sync::broadcast::Sender, task::JoinHandle};

pub(crate) async fn login(tx: Sender<Event>, args: Cli) -> Result<Client> {
    log::info!("Logging into Matrix homeserver...");
    let client = Client::builder()
        .homeserver_url(format!("https://{}", args.matrix_username.server_name()))
        .build()
        .await?;
    client
        .login_username(args.matrix_username.localpart(), &args.matrix_password)
        .initial_device_display_name("matrix-remote-closedown")
        .send()
        .await?;

    log::info!("Performing initial sync...");
    client.sync_once(SyncSettings::default()).await?;

    log::info!("Successfully logged in to Matrix homeserver");

    client.add_event_handler({
        let tx = tx.clone();
        move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let tx = tx.clone();
            async move {
                if let MessageType::Text(TextMessageEventContent { body, .. }) =
                    event.content.msgtype
                {
                    log::debug!("Received message in room {}", room.room_id());
                    crate::send_event!(
                        tx,
                        Event::MatrixMessageReceive(MatrixMessageReceiveEvent {
                            room: room.room_id().into(),
                            body,
                            sender: event.sender,
                        })
                    );
                }
            }
        }
    });

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
                        .send(RoomMessageEventContent::text_markdown(msg.body), None)
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
