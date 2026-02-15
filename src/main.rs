//! Main entrypoint for the EchoNet Lite to MQTT converter
mod config;
mod net;
mod echonet;

use clap::Parser;
use std::error::Error;
use std::io;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "A bridge between Echonet Lite (via an adapter) to MQTT", long_about = None)]
struct Args {
    #[arg(short, long, default_value = "/mnt/config/e2m.yaml", help = "Configuration file")]
    config: PathBuf
}

#[tokio::main(flavor = "multi_thread", worker_threads = 3)]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let args = Args::parse();

    // Read the config file
    let config_content = match std::fs::read_to_string(&args.config) {
        Ok(contents) => {
            contents
        }
        Err(e) => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("Unable to read the configuration file '{}': {}", args.config.display(), e),
            )))?;
        }
    };

    let config: config::AppConfiguration = match serde_yaml::from_str(&config_content) {
        Ok(config) => {
            config
        }
        Err(e) => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("Unable to parse the configuration file: {}", e),
            )))?;
        }
    };

    // Setup the network connections. There is a design limitation on this
    // approach. Even if the configuration specifies all interfaces, we must
    // bind to each interface individually. This is because each interface
    // may recieve an IPv6 all-hosts multicast, which is link local. Since
    // the API cannot determine which interface the packet was recieved from
    // we need to manage all connections individually. With that in mind, the
    // configuration interfaces section becomes a filter. If nothing spefified
    // then all interfaces, else only ones listed in the configuration. Start
    // by getting a list of interfaces. This is done here, before the network
    // startup so that we can detect configuration errors.
    let _ = net::get_network_interfaces().await;

    // Connect to MQTT

    // Start listening on the required interfaces.
    let _ = net::do_listen().await;
    
    Ok(())
}
