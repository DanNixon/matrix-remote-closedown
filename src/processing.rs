use crate::{
    command::Operation,
    event::{CommandEvent, Event},
    metrics::{CommandLables, COMMANDS},
    schema::{self, Response, Status},
    Cli,
};
use anyhow::Result;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use mqtt_channel_client as mqtt;
use tokio::{sync::broadcast::Sender, task::JoinHandle};
use unindent::Unindent;

macro_rules! format_optional_bool {
    ($v:expr, $str_true:expr, $str_false:expr, $str_none:expr) => {
        match $v {
            Some(true) => $str_true,
            Some(false) => $str_false,
            None => $str_none,
        }
    };
}

pub(crate) fn run_task(
    tx: Sender<Event>,
    mqtt_client: mqtt_channel_client::Client,
    matrix_client: matrix_sdk::Client,
    config: Cli,
) -> Result<JoinHandle<()>> {
    let mut rx = tx.subscribe();

    Ok(tokio::spawn(async move {
        let mut mqtt_rx = mqtt_client.rx_channel();

        let mut old_status = Status::default();

        loop {
            tokio::select! {
                Ok(event) = rx.recv() => {
                    match event {
                        Event::Exit => {
                            log::debug!("Task exit");
                            return;
                        }
                        Event::MqttSendCommandMessage(msg) => {
                            log::info!("Sending command message: {}", msg);
                            if let Err(e) = mqtt_client.send(mqtt::paho_mqtt::Message::new(&config.command_topic, msg, 2)) {
                                log::warn!("Error sending command message ({})", e);
                            }
                        },
                        Event::MatrixMessageReceive(event) => {
                            if !event.body.starts_with('!') {
                                log::debug!("Ignoring message with no command marker");
                                continue;
                            }

                            let room = event.room.clone();
                            if !config.matrix_rooms.contains(&room) {
                                log::debug!("Ignoring message in room we do not watch");
                                continue;
                            }

                            let sender = event.sender.clone();
                            if config.matrix_username == sender {
                                log::debug!("Ignoring message sent by the bot user");
                                continue;
                            }

                            log::info!("Message from Matrix: {}", event.body);
                            match event.try_into() {
                                Ok::<CommandEvent, _>(cmd_event) => {
                                    if cmd_event.cmd.station_name == config.station_name {
                                        crate::send_event!(tx, Event::CommandReceive(cmd_event));
                                    } else {
                                        log::debug!(
                                            "Ignoring command with unknown station name: {}",
                                            cmd_event.cmd.station_name
                                        );
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to parse command from message because {}", e);
                                    matrix_client
                                        .get_joined_room(&room)
                                        .unwrap()
                                        .send(
                                            RoomMessageEventContent::text_markdown(format!(
                                                "{}: That command failed, try `!{} help` for usage details",
                                                sender, config.station_name
                                            )),
                                            None,
                                        )
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                        Event::CommandReceive(event) => {
                            log::info!("Processing command: {:?}", event);
                            COMMANDS
                                .get_or_create(&CommandLables::new(event.cmd.op.clone()))
                                .inc();
                            match event.cmd.op {
                                Operation::Help => {
                                    matrix_client
                                        .get_joined_room(&event.room)
                                        .unwrap()
                                        .send(
                                            RoomMessageEventContent::text_markdown(format!(
                                                "
                                                [matrix-remote-closedown](https://github.com/DanNixon/matrix-remote-closedown) for station **{}**.<br>
                                                Usage: !{} COMMAND<br>
                                                Commands: help, shutdown, power on, power off, ptt enable, ptt disable",
                                                config.station_name,
                                                config.station_name,
                                                ).unindent()
                                            ),
                                            None,
                                        )
                                        .await
                                        .unwrap();
                                }
                                Operation::Shutdown => {
                                    send_command(
                                        &tx,
                                        schema::Command {
                                            enable_tx_power: Some(false),
                                            enable_ptt: Some(false),
                                        },
                                    );
                                }
                                Operation::PowerOn => {
                                    send_command(
                                        &tx,
                                        schema::Command {
                                            enable_tx_power: Some(true),
                                            enable_ptt: None,
                                        },
                                    );
                                }
                                Operation::PowerOff => {
                                    send_command(
                                        &tx,
                                        schema::Command {
                                            enable_tx_power: Some(false),
                                            enable_ptt: None,
                                        },
                                    );
                                }
                                Operation::PttEnable => {
                                    send_command(
                                        &tx,
                                        schema::Command {
                                            enable_tx_power: None,
                                            enable_ptt: Some(true),
                                        },
                                    );
                                }
                                Operation::PttDisable => {
                                    send_command(
                                        &tx,
                                        schema::Command {
                                            enable_tx_power: None,
                                            enable_ptt: Some(false),
                                        },
                                    );
                                }
                            }
                        }
                        Event::MqttStatusMessageReceived(msg) => match serde_json::from_str(&msg) {
                            Ok::<Response, _>(msg) => {
                                log::info!("Received response/status message {:?}", msg);

                                if old_status != msg.status {
                                    send_status_messages(
                                        &matrix_client,
                                        &config,
                                        &format!(
                                            "
                                            **{}** at {}<br>
                                            TX Power: [{}] [{}]<br>
                                            PTT: [{}] [{}]",
                                            config.station_name,
                                            msg.timestamp,
                                            format_optional_bool!(
                                                msg.status.tx_power_enabled,
                                                "ENABLED",
                                                "DISABLED",
                                                "unknown"
                                            ),
                                            format_optional_bool!(
                                                msg.status.tx_power_active,
                                                "ON",
                                                "OFF",
                                                "unknown"
                                            ),
                                            format_optional_bool!(
                                                msg.status.ptt_enabled,
                                                "ENABLED",
                                                "DISABLED",
                                                "unknown"
                                            ),
                                            format_optional_bool!(
                                                msg.status.ptt_active,
                                                "ON AIR",
                                                "IDLE",
                                                "unknown"
                                            ),
                                        )
                                        .unindent(),
                                    )
                                    .await;
                                    old_status = msg.status;
                                }

                                if let Some(m) = msg.message {
                                    send_status_messages(
                                        &matrix_client,
                                        &config,
                                        &format!(
                                            "
                                            **{}** at {}<br>
                                            Message: {}",
                                            config.station_name,
                                            msg.timestamp,
                                            m,
                                        )
                                        .unindent(),
                                    )
                                    .await;
                                }
                            }
                            Err(e) => {
                                log::warn!("Failed to parse response from MQTT message, because {}", e);
                            }
                        },
                    }
                },
                event = mqtt_rx.recv() => {
                    if let Ok(mqtt_channel_client::Event::Rx(msg)) = event {
                        crate::send_event!(
                            tx,
                            Event::MqttStatusMessageReceived(msg.payload_str().to_string())
                        );
                    }
                },
            }
        }
    }))
}

fn send_command(tx: &Sender<Event>, cmd: schema::Command) {
    match serde_json::to_string(&cmd) {
        Ok(cmd) => {
            crate::send_event!(tx, Event::MqttSendCommandMessage(cmd))
        }
        Err(e) => {
            log::error!("Failed to serialise command message because {}", e);
        }
    }
}

async fn send_status_messages(matrix_client: &matrix_sdk::Client, config: &Cli, body: &str) {
    for room in &config.matrix_rooms {
        matrix_client
            .get_joined_room(room)
            .unwrap()
            .send(RoomMessageEventContent::text_markdown(body), None)
            .await
            .unwrap();
    }
}
