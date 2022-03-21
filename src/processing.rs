use crate::{
    command::Operation,
    event::{CommandEvent, Event, MatrixMessageSendEvent},
    schema::{self, Response, Status},
    Cli,
};
use anyhow::Result;
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

pub(crate) fn run_task(tx: Sender<Event>, config: Cli) -> Result<JoinHandle<()>> {
    let mut rx = tx.subscribe();

    Ok(tokio::spawn(async move {
        let mut old_status = Status::default();

        while let Ok(event) = rx.recv().await {
            match event {
                Event::Exit => {
                    log::debug!("Task exit");
                    return;
                }
                Event::MatrixMessageReceive(event) => {
                    if !event.body.starts_with('!') {
                        continue;
                    }

                    let room = event.room.clone();
                    if !config.matrix_rooms.contains(&room) {
                        continue;
                    }

                    let sender = event.sender.clone();
                    if config.matrix_username == sender {
                        continue;
                    }

                    match event.try_into() {
                        Ok::<CommandEvent, _>(cmd_event) => {
                            if cmd_event.cmd.station_name == config.station_name {
                                let op_only_command = cmd_event.cmd.op.is_operator_only();
                                if !op_only_command || config.station_operators.contains(&sender) {
                                    crate::send_event!(tx, Event::CommandReceive(cmd_event));
                                } else {
                                    log::info!("Ignoring operator only command issued by {}, who is not an operator", sender);
                                }
                            } else {
                                log::debug!(
                                    "Ignoring command with unknown station name: {}",
                                    cmd_event.cmd.station_name
                                );
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to parse command from message because {}", e);
                            crate::send_event!(
                                tx,
                                Event::MatrixMessageSend(MatrixMessageSendEvent {
                                    room,
                                    body: format!(
                                        "{}: That command failed, try `!{} help` for usage details",
                                        sender, config.station_name
                                    ),
                                })
                            );
                        }
                    }
                }
                Event::CommandReceive(event) => {
                    log::info!("Processing command: {:?}", event);
                    match event.cmd.op {
                        Operation::Help => {
                            crate::send_event!(
                                tx,
                                Event::MatrixMessageSend(MatrixMessageSendEvent {
                                    room: event.room,
                                    body: format!(
                                        "
                                        [matrix-remote-closedown](https://github.com/DanNixon/matrix-remote-closedown) for station **{}**.<br>
                                        Usage: !{} COMMAND<br>
                                        Commands: help, shutdown, power on, power off, ptt enable, ptt disable<br>
                                        Station operators: {}",
                                        config.station_name,
                                        config.station_name,
                                        config.station_operators_string_list(),
                                        ).unindent(),
                                })
                            );
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
                                &tx,
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
                            );
                            old_status = msg.status;
                        }

                        if let Some(m) = msg.message {
                            send_status_messages(
                                &tx,
                                &config,
                                &format!(
                                    "
                                    ({})<br>
                                    **{}** at {}<br>
                                    Message: {}",
                                    config.station_operators_string_list(),
                                    config.station_name,
                                    msg.timestamp,
                                    m,
                                )
                                .unindent(),
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse response from MQTT message, because {}", e);
                    }
                },
                _ => {}
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

fn send_status_messages(tx: &Sender<Event>, config: &Cli, body: &str) {
    for room in &config.matrix_rooms {
        crate::send_event!(
            tx,
            Event::MatrixMessageSend(MatrixMessageSendEvent {
                room: room.clone(),
                body: body.to_string(),
            })
        );
    }
}
