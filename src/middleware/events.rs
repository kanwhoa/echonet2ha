//! Specific events to be sent in and out of the middleware

/// Events known to the middleware
pub enum Event {
    Startup,
    Shutdown,
    Announce
}

impl Clone for Event {
    fn clone(&self) -> Self {
        match self {
            Event::Startup => Event::Startup,
            Event::Shutdown => Event::Shutdown,
            Event::Announce => Event::Announce,
        }
    }
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Event::Startup => write!(f, "Startup"),
            Event::Shutdown => write!(f, "Shutdown"),
            Event::Announce => write!(f, "Announce"),
        }
    }
}

