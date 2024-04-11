use clap::Parser;

/// Query "nous A1Z" smart plugs exposed via Zigbee2MQTT, accumulate the data
/// and host it as JSON data using a simple web server so that it can be
/// imported into Grafana.
#[derive(Debug, Parser)]
#[command(version, about, long_about)]
struct Args {
    /// The MQTT server URL. Example: mqtt://localhost:1833
    server: String,
    /// Topic where the smart plugs are exposed under
    topic: String,
    /// Username for authorization, if applicable
    #[arg(long)]
    user: Option<String>,
    /// Password for authorization, if applicable
    #[arg(long)]
    pass: Option<String>,
    /// Friendly names of the smart plugs to query
    friendly_names: Vec<String>,
}

fn main() {
    let _args = Args::parse();

    println!("Hello, world!");
}
