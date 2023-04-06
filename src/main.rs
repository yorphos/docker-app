use docker_app::{
    listen_for_termination_signal, serve_requests_in_queue_metric, start_serving, Broker,
};
use std::env;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let broker = Broker {
        id: env::var("MQTT_BROKER_ID").unwrap(),
        host: env::var("MQTT_BROKER_HOST").unwrap(),
        port: env::var("MQTT_BROKER_PORT")
            .unwrap()
            .parse::<u16>()
            .unwrap(),
    };

    let requests_in_queue = RwLock::new(0);
    let requests_in_queue_arc = std::sync::Arc::new(requests_in_queue);

    let start_serving_future =
        start_serving(&broker, std::sync::Arc::clone(&requests_in_queue_arc));
    let serve_requests_in_queue_metric_future =
        serve_requests_in_queue_metric(requests_in_queue_arc);
    let termination_signal = listen_for_termination_signal();

    tokio::select! {
        _ = start_serving_future => (),
        _ = serve_requests_in_queue_metric_future => (),
        _ = termination_signal => (),
    }
}
