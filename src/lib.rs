use std::time::Duration;

use futures::{stream::FuturesUnordered, StreamExt};
use md5::{Digest, Md5};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_derive::{Deserialize, Serialize};
use tokio::task;

#[derive(Clone)]
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
    nonce: u32,
}

fn digest_leading_zeros(digest_bytes: &[u8]) -> u8 {
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

fn handle_request(data: Vec<u8>, leading_zeros: u8) -> u32 {
    let mut hasher = Md5::new();
    let mut nonce: u32 = 0;

    loop {
        hasher.update(&data);
        hasher.update(nonce.to_ne_bytes());

        let digest = hasher.finalize_reset();

        if digest_leading_zeros(digest.as_slice()) >= leading_zeros {
            return nonce;
        }
        nonce += 1;
    }
}

pub async fn start_serving(broker: &Broker) {
    let mut mqtt_options = MqttOptions::new(&broker.id, &broker.host, broker.port);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

    let qos = QoS::AtMostOnce;
    client.subscribe("hash/request", qos).await.unwrap();

    let mut tasks: FuturesUnordered<tokio::task::JoinHandle<()>> = FuturesUnordered::new();
    let mut iterations = 0;

    client
        .publish("hash/start", QoS::AtLeastOnce, false, [])
        .await
        .unwrap();

    loop {
        if let Ok(Event::Incoming(Packet::Publish(publish))) = eventloop.poll().await {
            if iterations < 512 {
                iterations += 1;
                let client_clone = client.clone();

                let task_handle = task::spawn(async move {
                    let request = postcard::from_bytes::<Request>(&publish.payload).unwrap();

                    let request_data = request.data.clone().to_vec();

                    let nonce = tokio::task::spawn_blocking(move || {
                        handle_request(request_data, request.leading_zeros)
                    })
                    .await
                    .unwrap();

                    let response = Response {
                        data: request.data,
                        leading_zeros: request.leading_zeros,
                        nonce,
                    };

                    let response_bytes = postcard::to_vec::<_, 256>(&response).unwrap().to_vec();

                    client_clone
                        .publish("hash/response", QoS::AtMostOnce, false, response_bytes)
                        .await
                        .unwrap()
                });

                tasks.push(task_handle);
            } else {
                break;
            }
        }
    }

    loop {
        tokio::select! {
            _ = eventloop.poll() => {
            },
            task = tasks.next() => {
                if let None = task {
                    break;
                }
            }
        }
    }
}
