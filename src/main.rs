mod command;
mod event;
mod matrix;
mod mqtt;
mod processing;
mod schema;

use crate::event::Event;
use anyhow::Result;
use clap::Parser;
use matrix_sdk::{
    ruma::{RoomId, UserId},
    SyncSettings,
};
use tokio::{signal, sync::broadcast};

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
    matrix_username: UserId,

    /// Matrix password
    #[clap(value_parser, long, env = "MATRIX_PASSWORD")]
    matrix_password: String,

    /// Topic to listen for status messages on
    #[clap(value_parser, long, env = "STATUS_TOPIC")]
    status_topic: String,

    /// Topic to send command messages on
    #[clap(value_parser, long, env = "COMMAND_TOPIC")]
    command_topic: String,

    /// Station name
    #[clap(value_parser, long, env = "STATION_NAME")]
    station_name: String,

    /// Station operator Matrix IDs
    #[clap(value_parser, long = "operator")]
    station_operators: Vec<UserId>,

    /// Matrix rooms to send messages to and listen for commands from
    #[clap(value_parser, long = "room")]
    matrix_rooms: Vec<RoomId>,
}

impl Cli {
    pub(crate) fn station_operators_string_list(&self) -> String {
        self.station_operators
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Cli::parse();

    let (tx, mut rx) = broadcast::channel::<Event>(16);

    let matrix_client = matrix::login(tx.clone(), args.clone()).await?;

    let tasks = vec![
        processing::run_task(tx.clone(), args.clone())?,
        mqtt::run_task(tx.clone(), &args).await?,
        matrix::run_send_task(tx.clone(), matrix_client.clone())?,
    ];

    let matrix_sync_task = tokio::spawn(async move {
        matrix_client
            .sync(SyncSettings::default().token(matrix_client.sync_token().await.unwrap()))
            .await;
    });

    loop {
        let should_exit = tokio::select! {
            _ = signal::ctrl_c() => true,
            event = rx.recv() => matches!(event, Ok(Event::Exit)),
        };
        if should_exit {
            break;
        }
    }

    log::info! {"Terminating..."};
    tx.send(Event::Exit)?;
    for handle in tasks {
        if let Err(e) = handle.await {
            log::error!("Failed waiting for task to finish: {}", e);
        }
    }
    matrix_sync_task.abort();
    let _ = matrix_sync_task.await;

    Ok(())
}
