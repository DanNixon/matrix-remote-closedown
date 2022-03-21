use crate::{event::Event, Cli};
use anyhow::Result;
use paho_mqtt::{AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, Message};
use std::env;
use tokio::{
    sync::broadcast::Sender,
    task::JoinHandle,
    time::{self, Duration},
};

pub(crate) async fn run_task(tx: Sender<Event>, args: &Cli) -> Result<JoinHandle<()>> {
    let mut client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(&args.mqtt_broker)
            .client_id(&args.mqtt_client_id)
            .persistence(env::temp_dir())
            .finalize(),
    )?;

    let stream = client.get_stream(25);

    client
        .connect(
            ConnectOptionsBuilder::new()
                .user_name(&args.mqtt_username)
                .password(&args.mqtt_password)
                .finalize(),
        )
        .wait()?;

    client.subscribe(&args.status_topic, args.mqtt_qos).await?;

    let mut rx = tx.subscribe();
    let command_topic = args.command_topic.clone();
    let qos = args.mqtt_qos;

    Ok(tokio::spawn(async move {
        let mut beat = time::interval(Duration::from_millis(100));

        loop {
            if let Ok(event) = rx.try_recv() {
                match event {
                    Event::Exit => {
                        log::debug!("Task exit");
                        return;
                    }
                    Event::MqttSendCommandMessage(msg) => {
                        match client.try_publish(Message::new(command_topic.clone(), msg, qos)) {
                            Ok(delivery_token) => {
                                if let Err(e) = delivery_token.wait() {
                                    log::error!("Error sending message: {}", e);
                                }
                            }
                            Err(e) => log::error!("Error creating/queuing the message: {}", e),
                        }
                    }
                    _ => {}
                }
            }

            if let Ok(Some(msg)) = stream.try_recv() {
                log::info! {"Received message on topic \"{}\"", msg.topic()};
                crate::send_event!(
                    tx,
                    Event::MqttStatusMessageReceived(msg.payload_str().to_string())
                );
            }

            beat.tick().await;
        }
    }))
}
