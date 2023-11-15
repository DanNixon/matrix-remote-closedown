mod command;
mod event;
mod metrics;
mod processing;
mod schema;

use crate::event::{Event, MatrixMessageReceiveEvent};
use anyhow::Result;
use clap::Parser;
use kagiyama::{AlwaysReady, Watcher};
use matrix_sdk::{
    event_handler::Ctx,
    room::Room,
    ruma::{
        events::room::message::{
            MessageType, OriginalSyncRoomMessageEvent, TextMessageEventContent,
        },
        OwnedRoomId, OwnedUserId,
    },
};
use mqtt_channel_client as mqtt;
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tokio::sync::broadcast;

#[macro_export]
macro_rules! send_event {
    ($tx:expr, $event:expr) => {
        if let Err(e) = $tx.send($event) {
            log::error!("Failed to send event: {}", e);
        }
    };
}

/// A Matrix bot that provides a nice interface to remote-closedown.
#[derive(Clone, Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Address of MQTT broker to connect to
    #[clap(
        value_parser,
        long,
        env = "MQTT_BROKER",
        default_value = "tcp://localhost:1883"
    )]
    mqtt_broker: String,

    /// Client ID to use when connecting to MQTT broker
    #[clap(
        value_parser,
        long,
        env = "MQTT_CLIENT_ID",
        default_value = "matrix-remote-closedown"
    )]
    mqtt_client_id: String,

    /// MQTT QoS, must be 0, 1 or 2
    #[clap(value_parser, long, env = "MQTT_QOS", default_value = "0")]
    mqtt_qos: i32,

    /// MQTT username
    #[clap(value_parser, long, env = "MQTT_USERNAME", default_value = "")]
    mqtt_username: String,

    /// MQTT password
    #[clap(value_parser, long, env = "MQTT_PASSWORD", default_value = "")]
    mqtt_password: String,

    /// Matrix username
    #[clap(value_parser, long, env = "MATRIX_USERNAME")]
    matrix_username: OwnedUserId,

    /// Matrix password
    #[clap(value_parser, long, env = "MATRIX_PASSWORD")]
    matrix_password: String,

    /// Matrix storage directory
    #[clap(value_parser, long, env = "MATRIX_STORAGE")]
    matrix_storage: PathBuf,

    /// Topic to listen for status messages on
    #[clap(value_parser, long, env = "STATUS_TOPIC")]
    status_topic: String,

    /// Topic to send command messages on
    #[clap(value_parser, long, env = "COMMAND_TOPIC")]
    command_topic: String,

    /// Station name
    #[clap(value_parser, long, env = "STATION_NAME")]
    station_name: String,

    /// Matrix rooms to send messages to and listen for commands from
    #[clap(value_parser, long = "room")]
    matrix_rooms: Vec<OwnedRoomId>,

    /// Address to listen on for observability/metrics endpoints
    #[clap(
        value_parser,
        long,
        env = "OBSERVABILITY_ADDRESS",
        default_value = "127.0.0.1:9090"
    )]
    observability_address: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    let mqtt_client = mqtt::Client::new(
        mqtt::paho_mqtt::create_options::CreateOptionsBuilder::new()
            .server_uri(&args.mqtt_broker)
            .client_id(&args.mqtt_client_id)
            .persistence(mqtt::paho_mqtt::PersistenceType::None)
            .finalize(),
        mqtt::ClientConfig::default(),
    )?;

    let mut watcher = Watcher::<AlwaysReady>::default();
    {
        let mut registry = watcher.metrics_registry();
        let registry = registry.sub_registry_with_prefix("matrixremoteclosedown");
        mqtt_client.register_metrics(registry);
        registry.register("commands", "Command requests", metrics::COMMANDS.clone());
    }
    watcher.start_server(args.observability_address).await;

    let (tx, _) = broadcast::channel::<Event>(16);

    mqtt_client.subscribe(
        mqtt::SubscriptionBuilder::default()
            .topic(args.status_topic.clone())
            .build()
            .unwrap(),
    );
    mqtt_client
        .start(
            mqtt::paho_mqtt::connect_options::ConnectOptionsBuilder::new()
                .clean_session(true)
                .automatic_reconnect(Duration::from_secs(1), Duration::from_secs(5))
                .keep_alive_interval(Duration::from_secs(5))
                .user_name(&args.mqtt_username)
                .password(&args.mqtt_password)
                .finalize(),
        )
        .await?;

    let matrix_client = matrix_client_boilerplate::Client::new(
        args.matrix_username.as_str(),
        &args.matrix_password,
        "matrix-remote-closedown",
        &args.matrix_storage,
    )
    .await?;
    matrix_client.initial_sync().await?;

    matrix_client.client().add_event_handler_context(tx.clone());
    matrix_client.client().add_event_handler(on_room_message);

    matrix_client.start_background_sync().await;

    let processing_task = processing::run_task(
        tx.clone(),
        mqtt_client,
        matrix_client.client().clone(),
        args.clone(),
    )?;

    tokio::signal::ctrl_c().await.unwrap();
    log::info! {"Terminating"};
    tx.send(Event::Exit)?;
    let _ = processing_task.await;

    Ok(())
}

async fn on_room_message(
    event: OriginalSyncRoomMessageEvent,
    room: Room,
    tx: Ctx<broadcast::Sender<Event>>,
) {
    if let Room::Joined(room) = room {
        if let MessageType::Text(TextMessageEventContent { body, .. }) = event.content.msgtype {
            log::debug!("Received message in room {}", room.room_id());

            crate::send_event!(
                tx,
                Event::MatrixMessageReceive(MatrixMessageReceiveEvent {
                    room: room.room_id().into(),
                    body,
                    sender: event.sender,
                })
            );

            if let Err(e) = room.read_receipt(&event.event_id).await {
                log::warn!("Failed to send read receipt ({})", e);
            }
        }
    }
}
