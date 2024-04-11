use clap::Parser;

lazy_static::lazy_static! {
    static ref CLI_ARGS: Args = Args::parse();
}

/// Query "nous A1Z" smart plugs exposed via Zigbee2MQTT, accumulate the data
/// and host it as JSON data using a simple web server so that it can be
/// imported into Grafana.
#[derive(Debug, Parser)]
#[command(version, about, long_about)]
struct Args {
    /// The MQTT server URL. Example: mqtt://localhost:1833
    pub server: String,
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
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _ = Args::parse(); // Ensure Args have been given correctly, lazy_static does not seem to invoke Args::parse() in a way which would halt execution if an argument is missing/incorrect
                           // We can use `CLI_ARGS` after this.
}
