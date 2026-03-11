//! Connectors module.
//! These convert ECHONET/HA messages to events for the middleware 
pub mod echonet;
pub mod error;
//pub mod echonet_serial;
//pub mod homeassistant_net;

pub type Result<T> = core::result::Result<T, error::ConnectorError>;

pub trait Connectable {
    async fn initialise(&self, config: &crate::config::AppConfiguration) -> Result<()>;
    async fn connect(&self) -> Result<()>;
    async fn disconnect(&self) -> Result<()>;

    async fn recv(&self) -> Result<crate::middleware::events::Event>;
    async fn send(&self, event: &crate::middleware::events::Event) -> Result<()>;
}