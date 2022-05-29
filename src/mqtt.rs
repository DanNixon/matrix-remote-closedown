use crate::{event::Event, Cli};
use anyhow::Result;
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

    let response = client
        .connect(
            ConnectOptionsBuilder::new()
                .clean_session(true)
                .automatic_reconnect(Duration::from_secs(1), Duration::from_secs(5))
                .keep_alive_interval(Duration::from_secs(5))
                .user_name(&args.mqtt_username)
                .password(&args.mqtt_password)
                .finalize(),
        )
        .wait()?;

    log::info!(
        "Using MQTT version {}",
        response.connect_response().unwrap().mqtt_version
    );

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
                log::info!("Received message on topic \"{}\"", msg.topic());
                crate::send_event!(
                    tx,
                    Event::MqttStatusMessageReceived(msg.payload_str().to_string())
                );
            }

            beat.tick().await;
        }
    }))
}
