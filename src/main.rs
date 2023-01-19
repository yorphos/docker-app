use docker_app::{deserialize_payload, handle_request, Broker};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, Publish, QoS};
use serde_json;
use std::env;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::task;

async fn async_main() {
    let broker = Broker {
        id: env::var("MQTT_BROKER_ID").unwrap(),
        host: env::var("MQTT_BROKER_HOST").unwrap(),
        port: env::var("MQTT_BROKER_PORT")
            .unwrap()
            .parse::<u16>()
            .unwrap(),
    };

    let mut mqtt_options = MqttOptions::new(broker.id, broker.host, broker.port);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

    let topic = env::var("MQTT_TOPIC").unwrap();
    let qos = QoS::AtMostOnce;
    client.subscribe(topic, qos).await.unwrap();

    let mut iterations = env::var("ITERATION_COUNT").unwrap().parse::<i32>().unwrap();
    while iterations > 0 {
        let event = eventloop.poll().await.unwrap();
        match event {
            Event::Incoming(packet) => {
                match packet {
                    Packet::Publish(publish) => {
                        let client_clone = client.clone();
                        task::spawn(async move {
                            match client_clone
                                .publish(
                                    topic,
                                    QoS::AtMostOnce,
                                    false,
                                    deserialize_payload(&publish.payload),
                                )
                                .await
                            {
                                Ok(_) => {}
                                Err(_) => panic!(),
                            };
                        });
                    }
                    _ => {}
                }
                iterations -= 1;
            }
            _ => {}
        }
    }
}

fn main() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async_main());
}
