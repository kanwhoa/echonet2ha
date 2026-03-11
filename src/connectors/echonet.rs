//! Network related functions and structs
use nix::{self, ifaddrs::InterfaceAddress, net::if_::{if_nametoindex, InterfaceFlags}, sys::socket::{AddressFamily, SockaddrLike, SockaddrStorage}};
use std::ffi::CString;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use tokio::net::UdpSocket;
use super::error::ConnectorError;

/// Constants
const DEFAULT_PORT: u16 = 3610;
const MULTICAST_IPV4: Ipv4Addr = Ipv4Addr::new(224, 0, 23, 0);
const MULTICAST_IPV6: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1);


/// Keep track of the open sockets.
pub struct InterfaceSocket {
    /// The network address. Can be either V4 or V6. Need to keep the Sin structure to get the scope id.
    pub address: SocketAddr,
    /// The network address. Can be either V4 or V6. Need to keep the Sin structure to get the scope id.s
    pub network: SocketAddr,
    /// The netmask. Store as an address to avoid constant re-expansion.
    pub netmask: SocketAddr,
    /// True if multicast is supported on this interface address
    pub multicast_capable: bool,
    /// True if multicast is selected for this interface addresss
    pub multicast_selected: bool,
    /// The bound socket
    pub socket: std::cell::RefCell<Option<tokio::net::UdpSocket>>
}

impl PartialEq for InterfaceSocket {
    fn eq(&self, other: &Self) -> bool {
        self.address.ip() == other.address.ip()
    }
}

impl Eq for InterfaceSocket {}

impl std::hash::Hash for InterfaceSocket {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

/// To keep track of the listen addresses and open sockets
pub struct InterfaceListenAddresses {
    /// The OS interface index (for OS reference)
    interface_index: u32,
    /// The interface that this is referring to
    pub interface_name: String,
    // The list of interface sockets
    pub sockets: Vec<InterfaceSocket>,
}

impl PartialEq for InterfaceListenAddresses {
    fn eq(&self, other: &Self) -> bool {
        self.interface_index == other.interface_index
    }
}

impl Eq for InterfaceListenAddresses {}

impl std::hash::Hash for InterfaceListenAddresses {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.interface_index.hash(state);
    }
}

/// Convert an address/netmask to a network address.
fn to_network(address: SocketAddr, netmask: SocketAddr) -> Result<SocketAddr, ConnectorError> {
    match (address, netmask) {
        (SocketAddr::V4(a), SocketAddr::V4(n)) => {
            let mut network_octets = a.ip().octets();
            let netmask_octets = n.ip().octets();

            for index in 0..network_octets.len() {
                network_octets[index] &= netmask_octets[index];
            } 

            Ok(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from_octets(network_octets), a.port())))
        }
        (SocketAddr::V6(a), SocketAddr::V6(n)) => {
            let mut network_octets = a.ip().octets();
            let netmask_octets = n.ip().octets();

            for index in 0..network_octets.len() {
                network_octets[index] &= netmask_octets[index];
            } 

            Ok(SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from_octets(network_octets), a.port(), a.flowinfo(), a.scope_id())))
        }
        _ => Err(ConnectorError::MismatchedTypes())
    }
}

/// Convert from Nix structures to std::net structures
/// Ugly... very ugly
fn sockaddr_nix_to_stdnet(maybe_storage: Option<SockaddrStorage>, port: u16) -> Option<SocketAddr> {
    maybe_storage.and_then(|storage|
        if let Some(sin) = storage.as_sockaddr_in() {
            Some(SocketAddrV4::new(sin.ip(), port).into())
        } else if let Some(sin) = storage.as_sockaddr_in6() {
            Some(SocketAddrV6::new(sin.ip(), port, sin.flowinfo(), sin.scope_id()).into())
        } else {
            None
        }
    )
}

/// Copied until is_global is stable
pub const fn is_globalish4(obj: &Ipv4Addr) -> bool {
    !(obj.octets()[0] == 0 // "This network"
        || obj.is_private()
        || obj.is_loopback()
        || obj.is_link_local()
        // addresses reserved for future protocols (`192.0.0.0/24`)
        // .9 and .10 are documented as globally reachable so they're excluded
        || (
            obj.octets()[0] == 192 && obj.octets()[1] == 0 && obj.octets()[2] == 0
            && obj.octets()[3] != 9 && obj.octets()[3] != 10
        )
        || obj.is_documentation()
        || obj.is_broadcast())
}

/// Copied until is_global is stable
pub const fn is_globalish6(obj: &Ipv6Addr) -> bool {
    !(obj.is_unspecified()
        || obj.is_loopback()
        // IPv4-mapped Address (`::ffff:0:0/96`)
        || matches!(obj.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
        // IPv4-IPv6 Translat. (`64:ff9b:1::/48`)
        || matches!(obj.segments(), [0x64, 0xff9b, 1, _, _, _, _, _])
        // Discard-Only Address Block (`100::/64`)
        || matches!(obj.segments(), [0x100, 0, 0, 0, _, _, _, _])
        // IETF Protocol Assignments (`2001::/23`)
        || (matches!(obj.segments(), [0x2001, b, _, _, _, _, _, _] if b < 0x200)
            && !(
                // Port Control Protocol Anycast (`2001:1::1`)
                u128::from_be_bytes(obj.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0001
                // Traversal Using Relays around NAT Anycast (`2001:1::2`)
                || u128::from_be_bytes(obj.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0002
                // AMT (`2001:3::/32`)
                || matches!(obj.segments(), [0x2001, 3, _, _, _, _, _, _])
                // AS112-v6 (`2001:4:112::/48`)
                || matches!(obj.segments(), [0x2001, 4, 0x112, _, _, _, _, _])
                // ORCHIDv2 (`2001:20::/28`)
                // Drone Remote ID Protocol Entity Tags (DETs) Prefix (`2001:30::/28`)`
                || matches!(obj.segments(), [0x2001, b, _, _, _, _, _, _] if b >= 0x20 && b <= 0x3F)
            ))
        // 6to4 (`2002::/16`) – it's not explicitly documented as globally reachable,
        // IANA says N/A.
        || matches!(obj.segments(), [0x2002, _, _, _, _, _, _, _])
        // Segment Routing (SRv6) SIDs (`5f00::/16`)
        || matches!(obj.segments(), [0x5f00, ..])
        || obj.is_unique_local()
        || obj.is_unicast_link_local())
}

/// Convert a message to wire format
fn to_wire(event: &crate::middleware::events::Event) -> super::Result<Vec<u8>> {
    Ok(Vec::new())
}

/// Network based communication
pub struct Network {
    /// Store for the addresses and sockets
    ilas: std::cell::RefCell<Option<Vec<InterfaceListenAddresses>>>
}

/// Implementation methods for Network
impl Network {
    pub fn new() -> Self {
        Self {
            ilas: std::cell::RefCell::new(None)
        }
    }

    /// Send an event to all multicast sockets
    fn send_multicast(&self, event: &crate::middleware::events::Event) -> super::Result<()> {
        let data = to_wire(event)?;

        Ok(())
    }
}

impl super::Connectable for Network {
    /// Get a list of addresses/interfaces o bind to.
    /// 
    /// There's a complexity in how this works. Since we allow the configuration
    /// to specify different interfaces, we must bind to each interface
    /// independently. This means we also need to keep track of the source
    /// interface only reply to that interface. For an interface to be selected
    /// it should have both the interface and an address of the IP version active.
    async fn initialise(&self, config: &crate::config::AppConfiguration) -> super::Result<()> {
        let mut maybe_only_interfaces: Option<std::collections::HashMap<&str, &crate::config::AppInterfaceConfiguration>> = None;
        let mut ilas: std::collections::HashSet<InterfaceListenAddresses> = std::collections::HashSet::new();


        // Create a list of interfaces to filter by. If maybe_only_interfaces is Some(_) then, one or more network interfaces are
        // specified, else all interfaces.
        if let Some(interfaces) = &(config.network.interfaces) {
            if interfaces.len() > 0 {
                maybe_only_interfaces = Some(interfaces.iter().map(|x| (x.name.as_ref(), x)).collect());
            }
        }

        // Create a set of interface addresses by interface/protocol to make things easy to search by
        // Scan through to select the appropriate interfaces. Need to do this as
        // the loopback interface can have other addresses. This does a lookup of
        // the name so it can be excluded later.
        for ifaddr in nix::ifaddrs::getifaddrs().unwrap() {
            let interface_name: String = ifaddr.interface_name.clone();
            if ifaddr.address.is_none() || !ifaddr.flags.contains(InterfaceFlags::IFF_UP) || !ifaddr.flags.contains(InterfaceFlags::IFF_RUNNING) || ifaddr.flags.contains(InterfaceFlags::IFF_LOOPBACK) {
                continue;
            }

            // Skip any PTP links
            if config.network.skip_ptp && ifaddr.flags.contains(InterfaceFlags::IFF_POINTOPOINT) {
                log::debug!("Skipping address on interface '{}' as skip_ptp is set", interface_name);
                continue;
            }

            // Check if we have an interface filter active, and this interface is included.
            if maybe_only_interfaces.as_ref().is_some_and(|hm| !hm.contains_key(interface_name.as_str())) {
                log::debug!("Skipping address interface '{}' as no configuration", interface_name);
                continue;
            }

            // Get the interface configuration (if provided)
            let maybe_interface_config = maybe_only_interfaces.as_ref().and_then(|e| e.get(interface_name.as_str()).cloned());
            let port = maybe_interface_config.and_then(|e| Some(e.port)).or_else(|| Some(DEFAULT_PORT)).unwrap();
            let maybe_address = sockaddr_nix_to_stdnet(ifaddr.address, port);
            if maybe_address.is_none() {
                log::debug!("Skipping address interface '{}' as not IPv4 or IPv6", interface_name);
                continue; // Address is no IPv4 or IPv6
            }
            let address = maybe_address.unwrap();
            let family_enabled = maybe_interface_config.and_then(|ic| 
                if matches!(address, SocketAddr::V4(_)) {
                    Some(ic.ipv4)
                } else if matches!(address, SocketAddr::V6(_)) {
                    Some(ic.ipv6)
                } else {
                    None
                }
            ).or_else(|| Some(true)).unwrap();

            // Check for the family being enabled and a second check for loopback
            if address.ip().is_loopback() || !family_enabled {
                continue;
            }
            
            // Get the interface index in case we need it
            let interface_name_cstr: CString = CString::new(interface_name.as_str()).expect("Null values in interface name");
            let interface_index = if_nametoindex(interface_name_cstr.as_c_str())?;

            // Find the correct interface struct. The API is painful here so have to create a dummy version and then do a contains/insert
            let temp_ila = InterfaceListenAddresses {
                interface_index: interface_index,
                interface_name: interface_name,
                sockets: Vec::new()
            };
            // Need to do a find because value has moved into the hashset.
            let mut ila = ilas.take(&temp_ila).or_else(|| Some(temp_ila)).unwrap();
            
            // Insert the new interface
            let netmask: SocketAddr = sockaddr_nix_to_stdnet(ifaddr.netmask, 0).unwrap();
            let is = InterfaceSocket {
                address: address,
                netmask: netmask,
                network: to_network(address, netmask).unwrap(),
                multicast_capable: ifaddr.flags.contains(InterfaceFlags::IFF_MULTICAST),
                multicast_selected: false,
                socket: std::cell::RefCell::new(None)
            };
            // Save the socket and re-insert
            ila.sockets.push(is);
            ilas.insert(ila);
        }

        // An interface can have multiple IP addresses.
        // Multicast is always site local (both for IPv4 and IPv6). However, we could be binding to multiple interfaces, which have the same multicast address. Therefore,
        // we cannot just bind to all the interfaces as the MLD or IGMP announcements would end up on the incorect interface. Therefore we need to pick an IPv4 & IPv6
        // address for each interface. This should really be link local (FE80::/10 or 169.254.0.0/16) address. This is fine for IPV6 with SLAAC, however, on IPv4,
        // no-one uses the link local addresses, and they are generally not assigned. Most use RFC5735 addresses, rarely gloal on internal networks).
        // Key principle is that we do not want more than one address on the same interface to listen for the multicast network, it would result in duplicate messages.
        // Additionally, we don't want to listen on two addresses in the same /64. In most cases, a SLAAC address will be assigned as the first address, ie. "secured"
        // and other addresses will be temporary addresses. Due to this, we want to assign the multicast listener to the first address on the interface that is link-local.
        // For other addresses, we want to listen for nuicast, but not multicast. This does make things interesting when replying as we potentially need to choose a 
        // different source address. Need to convert to a vector to get mutable references easily.
        let mut ilas_vec: Vec<InterfaceListenAddresses> = ilas.into_iter().collect();
        for ila in ilas_vec.iter_mut() {
            // Find the IPv4 multicast address (if any)
            let mut socket4_indexes: Vec<(usize, usize)> = ila.sockets.iter()
                .enumerate()
                .map(|(index, e)|
                    (
                        index,
                        index + if let IpAddr::V4(ip) = e.address.ip() && e.multicast_capable {
                            1 * (if ip.is_link_local() {1000} else if ip.is_private() {2000} else if is_globalish4(&ip) {3000} else {5000})
                        } else {
                            1_000_000
                        }
                    )
                )
                .filter(|&(_, pref)| pref < 1_000_000)
                .collect();
            socket4_indexes.sort_by_key(|&(_, pref)| pref);
            socket4_indexes.get(0).and_then(|&(index, _)| {
                ila.sockets[index].multicast_selected = true;
                Some(true)
            });

            // Find the IPv6 multicast address (if any)
            let mut socket6_indexes: Vec<(usize, usize)> = ila.sockets.iter()
                .enumerate()
                .map(|(index, e)|
                    (
                        index,
                        index + if let IpAddr::V6(ip) = e.address.ip() && e.multicast_capable {
                            1 * (if ip.is_unicast_link_local() {1000} else if ip.is_unique_local() {2000} else if is_globalish6(&ip) {3000} else {5000})
                        } else {
                            1_000_000
                        }
                    )
                )
                .filter(|&(_, pref)| pref < 1_000_000)
                .collect();
            socket6_indexes.sort_by_key(|&(_, pref)| pref);
            socket6_indexes.get(0).and_then(|&(index, _)| {
                ila.sockets[index].multicast_selected = true;
                Some(true)
            });
        }

        // Display the networking configuration
        for ila in ilas_vec.iter() {
            log::debug!("Interface '{}'", ila.interface_name);

            // Display the IPv4 addresses
            let addrs = ila.sockets.iter()
                .filter(|&e| matches!(e.address.ip(), IpAddr::V4(_)))
                .map(|e|
                    format!("{}:{}{}", e.address.ip().to_string(), e.address.port(), if e.multicast_selected {"(M)"} else {""})
                )
                .collect::<Vec<String>>().join(", ");
            log::debug!("  IPv4 addresses: {}", addrs);

            // Display the IPv4 addresses
            let addrs = ila.sockets.iter()
                .filter(|&e| matches!(e.address.ip(), IpAddr::V6(_)))
                .map(|e|
                    format!("{}:{}{}", e.address.ip().to_string(), e.address.port(), if e.multicast_selected {"(M)"} else {""})
                )
                .collect::<Vec<String>>().join(", ");
            log::debug!("  IPv6 addresses: {}", addrs);
        }

        // Save
        *self.ilas.borrow_mut() = Some(ilas_vec);

        Ok(())
    }

    /// Open all of the network connections
    async fn connect(&self) -> super::Result<()> {
        let empty = Vec::new();

        // Iterate through each and open the sockets
        for ila in self.ilas.borrow().as_ref().or_else(||Some(&empty)).unwrap() {
            for is in ila.sockets.iter() {
                log::info!("Opening {}:{}{} on interface '{}'", is.address.ip(), is.address.port(), if is.multicast_selected {" with multicast"} else {""}, ila.interface_name);

                // Create the socket
                let socket: UdpSocket = UdpSocket::bind(is.address).await?;

                // Enable multicast
                if is.multicast_selected {
                    if let IpAddr::V4(ip) = is.address.ip() {
                        socket.join_multicast_v4(MULTICAST_IPV4, ip)?;
                    } else if matches!(is.address, SocketAddr::V6(_)) {
                        socket.join_multicast_v6(&MULTICAST_IPV6, ila.interface_index)?;
                    }
                }

                // Save
                *is.socket.borrow_mut() = Some(socket);
            }
        }

        Ok(())
    }

    /// Close all sockets
    async fn disconnect(&self) -> super::Result<()> {
        let empty = Vec::new();

        for ila in self.ilas.borrow().as_ref().or_else(||Some(&empty)).unwrap() {
            for is in ila.sockets.iter() {
                log::info!("Closing {}:{}{} on interface '{}'", is.address.ip(), is.address.port(), if is.multicast_selected {" with multicast"} else {""}, ila.interface_name);
                
                // Enable multicast
                if let Some(socket) = is.socket.borrow().as_ref() && is.multicast_selected {
                    if let IpAddr::V4(ip) = is.address.ip() {
                        socket.leave_multicast_v4(MULTICAST_IPV4, ip)?;
                    } else if matches!(is.address, SocketAddr::V6(_)) {
                        socket.leave_multicast_v6(&MULTICAST_IPV6, ila.interface_index)?;
                    }
                }

                // Relase & close
                *is.socket.borrow_mut() = None;
            }
        }

        Ok(())
    }

    async fn recv(&self) -> super::Result<crate::middleware::events::Event> {
        todo!()
    }

    // Convert a message and send it
    async fn send(&self, event: &crate::middleware::events::Event) -> super::Result<()> {
        log::debug!("Sending message: {}", event);

        // Detect the event type and handle it.
        match event {
            crate::middleware::events::Event::Announce(_, _) => {
                // Send to all multicast sockets.
                self.send_multicast(event)?;
            }
            _ => {}
        };

        Ok(())
    }    
}
