use std::time::Duration;

use clap::Parser;
use rumqttc::{AsyncClient, MqttOptions};
use rusqlite::Connection;
use serde::Deserialize;

lazy_static::lazy_static! {
    static ref CLI_ARGS: Args = Args::parse();
}

/// Query "nous A1Z" smart plugs exposed via Zigbee2MQTT, accumulate the data
/// and host it as JSON data using a simple web server so that it can be
/// imported into Grafana.
#[derive(Debug, Parser)]
#[command(version, about, long_about)]
struct Args {
    /// The MQTT server URL. Example: mqtt://localhost
    pub server: String,
    /// The MQTT server Port. Example: 1833
    pub port: u16,
    /// Topic where the smart plugs are exposed under
    pub topic: String,
    /// Username for authorization, if applicable
    #[arg(long)]
    pub user: Option<String>,
    /// Password for authorization, if applicable
    #[arg(long)]
    pub pass: Option<String>,
    /// Friendly names of the smart plugs to query
    pub friendly_names: Vec<String>,
    /// Path to the SQLite database file. If not provided, the database will be created in the
    /// current working directory.
    #[arg(long)]
    pub db: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Response {
    child_lock: String,
    current: f32,
    device: Device,
    energy: f32,
    indicator_mode: String,
    linkquality: u8,
    power: u8,
    power_outage_memory: String,
    state: String,
    voltage: u8,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    application_version: u8,
    date_code: String,
    friendly_name: String,
    hardware_version: u8,
    ieee_addr: String,
    manufacturer_id: u16,
    manufacturer_name: String,
    model: String,
    network_address: u16,
    power_source: String,
    stack_version: u8,
    #[serde(rename = "type")]
    device_type: String,
    zcl_version: u8,
}
#[tokio::main]
async fn main() {
    let _ = Args::parse(); // Ensure Args have been given correctly, lazy_static does not seem to invoke Args::parse() in a way which would halt execution if an argument is missing/incorrect
                           // We can use `CLI_ARGS` after this.
    let path = CLI_ARGS.db.as_deref().unwrap_or("./zpowergraph.db");
    let conn = Connection::open(path).expect(
        format!(
            "Failed to open database at {}. Do you have write permissions for this path?",
            path
        )
        .as_str(),
    );
    let mut mqtt_options = MqttOptions::new("zpowergraph", &CLI_ARGS.server, CLI_ARGS.port);
    if let (Some(user), Some(pass)) = (&CLI_ARGS.user, &CLI_ARGS.pass) {
        mqtt_options.set_credentials(user, pass);
    }
    mqtt_options.set_keep_alive(Duration::from_secs(30));
    let (mut mqtt_client, mut eventloop) = AsyncClient::new(mqtt_options, 30);
    for friendly_name in CLI_ARGS.friendly_names.iter() {
        mqtt_client
            .subscribe(
                format!("{}/{}/get", &CLI_ARGS.topic, friendly_name),
                rumqttc::QoS::ExactlyOnce,
            )
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to subscribe to topic {}/{}/get",
                    &CLI_ARGS.topic, friendly_name
                )
            });
    }

    while let Ok(notification) = eventloop.poll().await {
        // TODO: Handle the notification, deserialize the JSON and store it in the database
        // TODO: Store different intervals of data with different resolutions. For example, store 1 minute data for 1 day, 1 hour data for 1 week, 1 day data for 1 month, 1 week data for 1 year.
    }
}
