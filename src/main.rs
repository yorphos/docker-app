use docker_app::{start_serving, Broker};
use std::env;

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

    start_serving(&broker).await
}
