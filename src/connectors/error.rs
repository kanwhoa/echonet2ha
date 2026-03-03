//! Errors for network and wire representations

/// Network configuration error
#[derive(Debug)]
pub enum ConnectorError {
    /// Error in the configuration
    ConfigurationError(String),
    /// When V4 and V6 addresses are intermixed, should not happen.
    MismatchedTypes(),
    /// When the underlying socket throws an IO error
    IOError(String),
    /// Geenral UNIX OS errors. The above IO error is from the std:: library
    OSError(&'static str)
}

// Error traits
impl std::fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConnectorError::ConfigurationError(msg) => write!(f, "Network configuration error: {}", msg),
            ConnectorError::MismatchedTypes() => write!(f, "Mismatched network types"),
            ConnectorError::IOError(msg) => write!(f, "IO error on socket: {}", msg),
            ConnectorError::OSError(msg) => write!(f, "OS error: {}", msg),
        }
    }
}

impl std::error::Error for ConnectorError {}

impl From<std::io::Error> for ConnectorError {
    fn from(value: std::io::Error) -> Self {
        ConnectorError::IOError(value.to_string())
    }
}

impl From<nix::errno::Errno> for ConnectorError {
    fn from(value: nix::errno::Errno) -> Self {
        ConnectorError::OSError(value.desc())
    }
}
