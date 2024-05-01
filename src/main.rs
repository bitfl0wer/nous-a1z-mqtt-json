// Copyright (c) 2024 bitfl0wer
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::time::{self, Duration, UNIX_EPOCH};

use anyhow::Result;
use clap::Parser;
use log::*;
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions};
use sea_query::{ColumnDef, Iden, Query, SqliteQueryBuilder, Table};
use sea_query_binder::SqlxBinder;
use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

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
    current: f32,
    device: Device,
    energy: f32,
    power: u16,
    state: Option<String>,
    voltage: u16,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    friendly_name: String,
    ieee_addr: Option<String>,
    manufacturer_id: Option<u16>,
    manufacturer_name: Option<String>,
    model: Option<String>,
}

#[derive(Iden)]
pub enum DeviceTable {
    Table,
    Id,
    FriendlyName,
    Timestamp,
    Current,
    Energy,
    Power,
    Voltage,
}

async fn initialize_database() -> Result<SqlitePool> {
    let path = CLI_ARGS.db.as_deref().unwrap_or("./zpowergraph.db");
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    trace!("Opened database connection");
    Ok(pool)
}

async fn initialize_table(pool: &SqlitePool) -> Result<()> {
    let table = Table::create()
        .table(DeviceTable::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(DeviceTable::Id)
                .integer()
                .auto_increment()
                .primary_key(),
        )
        .col(
            ColumnDef::new(DeviceTable::FriendlyName)
                .string()
                .not_null(),
        )
        .col(
            ColumnDef::new(DeviceTable::Timestamp)
                .timestamp()
                .not_null(),
        )
        .col(ColumnDef::new(DeviceTable::Current).float().not_null())
        .col(ColumnDef::new(DeviceTable::Energy).float().not_null())
        .col(ColumnDef::new(DeviceTable::Power).integer().not_null())
        .col(ColumnDef::new(DeviceTable::Voltage).integer().not_null())
        .build(SqliteQueryBuilder);

    sqlx::query(&table).execute(pool).await?;
    trace!("Executed database statement: {:?}", table);
    Ok(())
}

async fn connect_mqtt_client() -> Result<(AsyncClient, EventLoop)> {
    let mut mqtt_options: MqttOptions =
        MqttOptions::new("zpowergraph", &CLI_ARGS.server, CLI_ARGS.port);
    if let (Some(user), Some(pass)) = (&CLI_ARGS.user, &CLI_ARGS.pass) {
        mqtt_options.set_credentials(user, pass);
    }
    trace!("Set MQTT options: {:?}", mqtt_options);
    mqtt_options.set_keep_alive(Duration::from_secs(30));
    let (mqtt_client, eventloop) = AsyncClient::new(mqtt_options, 30);
    trace!("Created MQTT client and event loop");
    for friendly_name in CLI_ARGS.friendly_names.iter() {
        let result = mqtt_client
            .subscribe(
                format!("{}/{}", &CLI_ARGS.topic, friendly_name),
                rumqttc::QoS::ExactlyOnce,
            )
            .await;
        if let Err(error) = result {
            error!(
                "Error subscribing to topic {}/{}: {:?}",
                &CLI_ARGS.topic, friendly_name, error
            );
            panic!();
        }
        trace!("Subscribed to topic {}/{}", &CLI_ARGS.topic, friendly_name);
    }
    Ok((mqtt_client, eventloop))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse(); // Ensure Args have been given correctly, lazy_static does not seem to invoke Args::parse() in a way which would halt execution if an argument is missing/incorrect
                              // We can use `CLI_ARGS` after this.
    debug!("Parsed CLI arguments: {:?}", args);

    let pool = initialize_database().await?;
    initialize_table(&pool).await?;
    let (_, mut eventloop) = connect_mqtt_client().await?;

    loop {
        let notification = match eventloop.poll().await {
            Ok(notification) => notification,
            Err(e) => {
                error!("Error polling event loop: {:?}", e);
                continue;
            }
        };
        trace!("Received notification: {:?}", notification);
        if let Event::Incoming(Incoming::Publish(packet)) = notification {
            let payload = packet.payload;
            let response: Response = serde_json::from_slice(&payload)?;
            trace!("Deserialized response: {:#?}", response);
            info!(
                "Received data for device {}: current: {}, energy: {}, power: {}, voltage: {}",
                response.device.friendly_name,
                response.current,
                response.energy,
                response.power,
                response.voltage
            );
            let (sql, value) = Query::insert()
                .into_table(DeviceTable::Table)
                .columns([
                    DeviceTable::FriendlyName,
                    DeviceTable::Timestamp,
                    DeviceTable::Current,
                    DeviceTable::Energy,
                    DeviceTable::Power,
                    DeviceTable::Voltage,
                ])
                .values_panic([
                    response.device.friendly_name.into(),
                    time::SystemTime::now()
                        .duration_since(UNIX_EPOCH)?
                        .as_secs()
                        .into(),
                    response.current.into(),
                    response.energy.into(),
                    response.power.into(),
                    response.voltage.into(),
                ])
                .build_sqlx(SqliteQueryBuilder);
            sqlx::query_with(&sql, value).execute(&pool).await?;
        }

        // TODO: Store different intervals of data with different resolutions. For example, store 1 minute data for 1 day, 1 hour data for 1 week, 1 day data for 1 month, 1 week data for 1 year.
        // Run cleanup every day.
    }
}
