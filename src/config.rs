use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfiguration {
    #[serde(default)]
    pub logging: AppLoggingConfiguration,
    #[serde(default)]
    pub mqtt: AppMqttConfiguration,
    #[serde(default)]
    pub network: AppNetworkConfiguration,
}

/// Logging configuration.
#[derive(Debug, Deserialize)]
pub struct AppLoggingConfiguration {
    /*
    /// The logging level. One of DEBUG/INFO/WARN/ERROR
    pub level: String - fixme: enum
    /// The filename to log into. Templated by strftime.
    pub file: String
    */
}

#[derive(Debug, Deserialize)]
pub struct AppMqttConfiguration {
    /*
    pub host: String,
    pub port: u16,
    pub useMdns: bool,
    pub credentials: AppMqttCredentialsConfiguration,
    pub certificate: AppMqttCertificateConfiguration,
    pub truststore: AppMqttCertificateConfiguration,
    */
}

/// Network configuration. If not specified, the default will be to listen on
/// all interfaces, all protocols.
#[derive(Debug, Deserialize)]
pub struct AppNetworkConfiguration {
    #[serde(default)]
    interfaces: Option<Vec<AppInterfaceConfiguration>>
}

/// Interface configuration. 
#[derive(Debug, Deserialize)]
pub struct AppInterfaceConfiguration {
    name: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    ipv4: bool,
    #[serde(default)]
    ipv6: bool,
}


impl Default for AppConfiguration {
    fn default() -> Self {
        AppConfiguration {
            logging: Default::default(),
            mqtt: Default::default(),
            network: Default::default()
        }
    }
}

impl Default for AppLoggingConfiguration {
    fn default() -> Self {
        AppLoggingConfiguration {
        }
    }
}

impl Default for AppMqttConfiguration {
    fn default() -> Self {
        AppMqttConfiguration {
        }
    }
}

impl Default for AppNetworkConfiguration {
    fn default() -> Self {
        AppNetworkConfiguration {
            interfaces: None
        }
    }
}

impl Default for AppInterfaceConfiguration {
    fn default() -> Self {
        AppInterfaceConfiguration {
            name: Default::default(),
            port: 3610,
            ipv4: true,
            ipv6: true
        }
    }
}


