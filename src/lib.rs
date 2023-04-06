use std::time::Duration;
use std::{convert::Infallible, sync::Arc};

use futures::{stream::FuturesUnordered, StreamExt};
use md5::{Digest, Md5};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_derive::{Deserialize, Serialize};
use tokio::{signal, sync::RwLock, task};
use warp::Filter;

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

pub async fn listen_for_termination_signal() -> Result<(), Box<dyn std::error::Error>> {
    signal::ctrl_c().await?;
    Ok(())
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

pub async fn start_serving(broker: &Broker, requests_in_queue: Arc<RwLock<usize>>) {
    let mut mqtt_options = MqttOptions::new(&broker.id, &broker.host, broker.port);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

    let qos = QoS::AtMostOnce;
    client.subscribe("hash/request", qos).await.unwrap();
    client.subscribe("cmd/terminate", qos).await.unwrap();

    let mut tasks: FuturesUnordered<tokio::task::JoinHandle<()>> = FuturesUnordered::new();
    let mut terminate_received = false;

    client
        .publish("hash/start", QoS::AtLeastOnce, false, [])
        .await
        .unwrap();

    while !terminate_received {
        tokio::select! {
            Ok(event) = eventloop.poll() => {
                if let Event::Incoming(Packet::Publish(publish)) = event {
                    if publish.topic == "hash/request" {
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

                        {
                            let mut reqs = requests_in_queue.write().await;
                            *reqs += 1;
                        }
                    } else if publish.topic == "cmd/terminate" {
                        terminate_received = true;
                    }
                }
            },
            task = tasks.next() => {
                if let Some(_) = task {
                    {
                        let mut reqs = requests_in_queue.write().await;
                        *reqs -= 1;
                    }
                }
            }
        }
    }

    // Process remaining tasks
    while let Some(_) = tasks.next().await {
        {
            let mut reqs = requests_in_queue.write().await;
            *reqs -= 1;
        }
    }
}

pub async fn serve_requests_in_queue_metric(requests_in_queue: Arc<RwLock<usize>>) {
    let requests_in_queue_filter = warp::path("requests_in_queue").and(warp::any().and_then({
        let requests_in_queue = Arc::clone(&requests_in_queue);
        move || {
            let requests_in_queue = Arc::clone(&requests_in_queue);
            async move {
                let reqs = requests_in_queue.read().await;
                Ok::<_, Infallible>(format!("{}", *reqs))
            }
        }
    }));
    let routes = requests_in_queue_filter;
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
}
