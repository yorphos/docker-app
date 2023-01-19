use std::time::Duration;

use md5::{Digest, Md5};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_derive::{Deserialize, Serialize};
use tokio::task;

pub struct Broker {
    pub id: String,
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize)]
struct Request<'a> {
    data: &'a [u8],
    leading_zeros: u8,
}

#[derive(Serialize)]
struct Response<'a> {
    data: &'a [u8],
    leading_zeros: u8,
    nonce: u16,
}

fn digest_leading_zeroes(digest_bytes: &[u8]) -> u8 {
    let mut leading_zeros = 0;

    for byte in digest_bytes {
        let byte_leading_zeros = byte.leading_zeros();

        leading_zeros += byte_leading_zeros;

        if byte_leading_zeros != 8 {
            return leading_zeros as u8;
        }
    }

    leading_zeros as u8
}

fn handle_request<'a>(request: &'a Request) -> Response<'a> {
    let mut hasher = Md5::new();
    let mut nonce: u16 = 0;

    loop {
        hasher.update(request.data);
        hasher.update(nonce.to_ne_bytes());

        let digest = hasher.finalize_reset();

        if digest_leading_zeroes(digest.as_slice()) >= request.leading_zeros {
            let Request {
                data,
                leading_zeros,
            } = request;

            return Response {
                data,
                leading_zeros: *leading_zeros,
                nonce,
            };
        }
        nonce += 1;
    }
}

fn deserialize_payload(payload: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&handle_request(&serde_json::from_slice(&payload).unwrap())).unwrap()
}

pub async fn start_serving(broker: &Broker) -> () {
    let mut mqtt_options = MqttOptions::new(&broker.id, &broker.host, broker.port);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

    let qos = QoS::AtMostOnce;
    client.subscribe("hash/request", qos).await.unwrap();

    let mut iterations = 512;
    while iterations > 0 {
        let event = eventloop.poll().await.unwrap();
        match event {
            Event::Incoming(packet) => match packet {
                Packet::Publish(publish) => {
                    let client_clone = client.clone();
                    task::spawn(async move {
                        client_clone
                            .publish(
                                "hash/response",
                                QoS::AtMostOnce,
                                false,
                                deserialize_payload(&publish.payload),
                            )
                            .await
                            .unwrap()
                    });
                    iterations -= 1;
                }
                _ => {}
            },
            _ => {}
        }
    }
}
