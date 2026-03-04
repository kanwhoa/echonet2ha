//! Specific events to be sent in and out of the middleware

/// Events known to the middleware
pub enum Event {
    Startup,
    Shutdown,
    Announce(super::api::EOJ, super::api::EOJ)
}

impl Clone for Event {
    fn clone(&self) -> Self {
        match self {
            Event::Startup => Event::Startup,
            Event::Shutdown => Event::Shutdown,
            Event::Announce(seoj, deoj) => Event::Announce(seoj.clone(), deoj.clone()),
        }
    }
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Event::Startup => write!(f, "Startup"),
            Event::Shutdown => write!(f, "Shutdown"),
            Event::Announce(seoj, deoj) => display_message(f, "*", seoj, "multicast", deoj)
        }
    }
}

/// Display helper for ECHONET Lite events
fn display_message(f: &mut std::fmt::Formatter, src_ip: &str, seoj: &super::api::EOJ, dst_ip: &str, deoj: &super::api::EOJ) -> std::fmt::Result {
    let seoj_display: super::api::GroupClass = seoj.into();
    let deoj_display: super::api::GroupClass = deoj.into();
    write!(f, "Announce {}/{} ({}) -> {}/{} ({})", src_ip, seoj, seoj_display, dst_ip, deoj, deoj_display)
}


