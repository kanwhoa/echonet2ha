//! Connectors module.
//! These convert ECHONET/HA messages to events for the middleware 
pub mod echonet;
pub mod error;
//pub mod echonet_serial;
//pub mod homeassistant_net;

pub type Result<T> = core::result::Result<T, error::ConnectorError>;

trait Connectable {
    async fn initialise(config: &crate::config::AppConfiguration) -> Self;
    async fn connect() -> Result<()>;
    async fn disconnect() -> Result<()>;

    async fn recv() -> Result<crate::middleware::events::Event>;
    async fn send(event: &crate::middleware::events::Event) -> Result<()>;
}