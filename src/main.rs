//! Main entrypoint for the EchoNet Lite to MQTT converter
extern crate macros;
mod config;
mod middleware;

use clap::Parser;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{self, Duration, Instant};
use tokio::sync::mpsc;
use middleware::Middleware;
use middleware::events::Event;

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

    // Configure logging
    stderrlog::new()
        .module(module_path!())
        .verbosity(stderrlog::LogLevelNum::Debug)
        .timestamp(stderrlog::Timestamp::Second)
        .init()
        .unwrap();
    log::info!("ECHONET Lite <-> Home Assistant bridge vTODO starting");

    // Termination signals
    #[cfg(unix)]
    let mut sig_int = signal(SignalKind::interrupt())?;
    #[cfg(unix)]
    let mut sig_term = signal(SignalKind::terminate())?;

    // Timer signal for broadcast announcements
    let mut interval = time::interval(Duration::from_secs(600));
    interval.tick().await;

    // Create the broadcast channel. When a message is recieved on this, it will be broadcasted to all
    // ECHONET Lite nodes.
    let (broadcast_tx, mut broadcast_rx) = mpsc::channel::<Event>(16);

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
    //let _ = net::get_network_interfaces().await;

    // Startup the Middleware. We only do this after all the channels have been
    // setup and any authentication performed.
    let middleware = Middleware::new(1, broadcast_tx);
    middleware.initialise().await?;
    
    // Run the main processing loop
    loop {
        tokio::select! {
            // Termination signals
            _ = sig_int.recv() => {
                log::info!("Recieved SIGINT, exiting");
                middleware.shutdown().await?;
            }
            _ = sig_term.recv() => {
                log::info!("Recieved SIGTERM, exiting");
                middleware.shutdown().await?;
            }

            // Announcement timer
            _ = interval.tick() => {
                log::info!("Performing announcement");
                //middleware.announce().await;
            }

            // Recieved a broadcast message, advertise on all channels (maybe)
            // TODO: Do we need to handle None()? I.e. a comms failure, and shutdown
            Some(msg) = broadcast_rx.recv() => {
                match msg {
                    Event::Startup => {
                        log::debug!("Middleware startup complete");
                    }
                    Event::Shutdown => {
                        log::debug!("Middleware shutdown complete");
                        // TODO: how to make sure the queue has been flushed?
                        break;
                    }
                    Event::Announce => {
                        // FIXME: need the address, in this case, broadcast and data
                        log::info!("Performing announce");
                    }
                }
            }
        }
    }    
    
    Ok(())
}
