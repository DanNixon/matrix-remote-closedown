use crate::{event::Event, Cli};
use anyhow::{anyhow, Result};
use paho_mqtt::{
    AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, Message, PersistenceType,
};
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
            .persistence(PersistenceType::None)
            .finalize(),
    )?;

    let stream = client.get_stream(25);

    let status_topic = args.status_topic.clone();
    let mqtt_qos = args.mqtt_qos;
    client.set_connected_callback(move |c| {
        c.subscribe(&status_topic, mqtt_qos);
    });

    client
        .connect(
            ConnectOptionsBuilder::new()
                .clean_session(true)
                .user_name(&args.mqtt_username)
                .password(&args.mqtt_password)
                .finalize(),
        )
        .wait()?;

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

            match stream.try_recv() {
                Ok(Some(msg)) => {
                    log::info!("Received message on topic \"{}\"", msg.topic());
                    crate::send_event!(
                        tx,
                        Event::MqttStatusMessageReceived(msg.payload_str().to_string())
                    );
                }
                Ok(None) => {
                    if let Err(e) = try_reconnect(&client).await {
                        log::error!("Failed to reconnect: {}", e);
                        tx.send(Event::Exit).unwrap();
                    }
                }
                Err(_) => {}
            }

            beat.tick().await;
        }
    }))
}

async fn try_reconnect(c: &AsyncClient) -> Result<()> {
    for i in 0..10 {
        log::info!("Attempting reconnection {}...", i);
        match c.reconnect().await {
            Ok(_) => {
                log::info!("Reconnection successful");
                return Ok(());
            }
            Err(e) => {
                log::error!("Reconnection failed: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(anyhow!("Failed to reconnect to broker"))
}
