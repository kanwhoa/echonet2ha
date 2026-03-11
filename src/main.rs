//! Main entrypoint for the EchoNet Lite to Home Assistant bridge
#![feature(assert_matches)]
extern crate macros;
mod config;
mod connectors;
mod middleware;

use clap::Parser;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{self, Duration, Instant};
use tokio::sync::mpsc;

use connectors::Connectable;
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
    let network_sockets = connectors::echonet::Network::new();
    network_sockets.initialise(&config).await?;
    network_sockets.connect().await?;

    // Startup the Middleware. We only do this after all the channels have been
    // setup and any authentication performed.
    let middleware = Middleware::new(1, broadcast_tx);
    middleware.startup().await?;
    
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
            Some(event) = broadcast_rx.recv() => {
                match event {
                    Event::Startup => {
                        log::debug!("Middleware startup complete");
                    }
                    Event::Shutdown => {
                        log::debug!("Middleware shutdown complete");
                        // TODO: how to make sure the queue has been flushed?
                        break;
                    }
                    Event::Announce(_, _) => {
                        network_sockets.send(&event).await?;
                    }
                }
            }
        }
    }   

    // Close all the sockets
    network_sockets.disconnect().await?; 
    
    Ok(())
}


/* Approch 1

use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use futures::stream::SelectAll;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut socket_list = Vec::new();
    // Assuming you have a list of addresses
    let addrs = vec!["127.0.0.1:8080", "127.0.0.1:8081"]; 

    for addr in addrs {
        let stream = TcpStream::connect(addr).await?;
        // Map each socket to a stream of packets/data
        socket_list.push(Box::pin(async_stream::stream! {
            let mut buf = vec![0; 1024];
            loop {
                // Simplified reading logic
                yield stream.read(&mut buf).await;
            }
        }));
    }

    // Combine all streams
    let mut all_streams = SelectAll::new();
    for socket in socket_list {
        all_streams.push(socket);
    }

    // Select! over the merged stream
    loop {
        tokio::select! {
            Some(result) = all_streams.next() => {
                println!("Received data: {:?}", result);
            }
        }
    }
}

*/

/* Approach 2

use tokio::net::UdpSocket;
use tokio::time::{interval, Duration};
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // 1. Setup multiple sockets
    let socket1 = Arc::new(UdpSocket::bind("127.0.0.1:8081").await?);
    let socket2 = Arc::new(UdpSocket::bind("127.0.0.1:8082").await?);
    let sockets = vec![socket1, socket2];

    // 2. Create interval timer
    let mut interval = interval(Duration::from_secs(1));

    // 3. Prepare futures for sockets
    let mut futures = FuturesUnordered::new();
    for socket in sockets {
        let sock = socket.clone();
        futures.push(async move {
            let mut buf = [0; 1024];
            let (len, addr) = sock.recv_from(&mut buf).await?;
            Ok::<(usize, std::net::SocketAddr, Arc<UdpSocket>), std::io::Error>((len, addr, sock))
        });
    }

    loop {
        tokio::select! {
            // Handle socket messages
            Some(result) = futures.next() => {
                match result {
                    Ok((len, addr, sock)) => {
                        println!("Received {} bytes from {}", len, addr);
                        // Repush to keep listening on this socket
                        let s = sock.clone();
                        futures.push(async move {
                            let mut buf = [0; 1024];
                            let (len, addr) = s.recv_from(&mut buf).await?;
                            Ok::<(usize, std::net::SocketAddr, Arc<UdpSocket>), std::io::Error>((len, addr, s))
                        });
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            // Handle timer
            _ = interval.tick() => {
                println!("Timer ticked");
            }
        }
    }
}


*/