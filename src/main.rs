// Copyright (c) 2024 bitfl0wer
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::time::Duration;

use clap::Parser;
use log::*;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions};
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
    child_lock: Option<String>,
    current: Option<f32>,
    device: Option<Device>,
    energy: Option<f32>,
    indicator_mode: Option<String>,
    linkquality: Option<u8>,
    power: Option<u16>,
    power_outage_memory: Option<String>,
    state: Option<String>,
    voltage: Option<u16>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    application_version: Option<u8>,
    date_code: Option<String>,
    friendly_name: Option<String>,
    hardware_version: Option<u8>,
    ieee_addr: Option<String>,
    manufacturer_id: Option<u16>,
    manufacturer_name: Option<String>,
    model: Option<String>,
    network_address: Option<u16>,
    power_source: Option<String>,
    stack_version: Option<u8>,
    #[serde(rename = "type")]
    device_type: Option<String>,
    zcl_version: Option<u8>,
}
#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse(); // Ensure Args have been given correctly, lazy_static does not seem to invoke Args::parse() in a way which would halt execution if an argument is missing/incorrect
                              // We can use `CLI_ARGS` after this.
    debug!("Parsed CLI arguments: {:?}", args);
    let path = CLI_ARGS.db.as_deref().unwrap_or("./zpowergraph.db");
    let conn = Connection::open(path).unwrap_or_else(|_| {
        panic!(
            "Failed to open database at {}. Do you have write permissions for this path?",
            path
        )
    });
    trace!("Opened database connection");
    let mut mqtt_options: MqttOptions =
        MqttOptions::new("zpowergraph", &CLI_ARGS.server, CLI_ARGS.port);
    if let (Some(user), Some(pass)) = (&CLI_ARGS.user, &CLI_ARGS.pass) {
        mqtt_options.set_credentials(user, pass);
    }
    trace!("Set MQTT options: {:?}", mqtt_options);
    mqtt_options.set_keep_alive(Duration::from_secs(30));
    let (mut mqtt_client, mut eventloop) = AsyncClient::new(mqtt_options, 30);
    trace!("Created MQTT client and event loop");
    for friendly_name in CLI_ARGS.friendly_names.iter() {
        mqtt_client
            .subscribe(
                format!("{}/{}", &CLI_ARGS.topic, friendly_name),
                rumqttc::QoS::ExactlyOnce,
            )
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to subscribe to topic {}/{}",
                    &CLI_ARGS.topic, friendly_name
                )
            });
        trace!("Subscribed to topic {}/{}", &CLI_ARGS.topic, friendly_name);
    }

    loop {
        let notification = match eventloop.poll().await {
            Ok(notification) => notification,
            Err(e) => {
                error!("Error polling event loop: {:?}", e);
                continue;
            }
        };
        debug!("Received notification: {:?}", notification);
        // TODO: Handle the notification, deserialize the JSON and store it in the database
        if let Event::Incoming(Incoming::Publish(packet)) = notification {
            let payload = packet.payload;
            let response: Response = serde_json::from_slice(&payload).unwrap();
            debug!("Deserialized response: {:?}", response);
        }

        // TODO: Store different intervals of data with different resolutions. For example, store 1 minute data for 1 day, 1 hour data for 1 week, 1 day data for 1 month, 1 week data for 1 year.
    }
}
