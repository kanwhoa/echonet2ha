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
pub struct InterfaceSocket<T> {
    /// A unique identifier for the interface address
    pub interface_address_index: usize,
    /// The network address. Can be either V4 or V6. Need to keep the Sin structure to get the scope id.
    pub address: T,
    /// The network address. Can be either V4 or V6. Need to keep the Sin structure to get the scope id.s
    pub network: T,
    /// The netmask. Store as an address to avoid constant re-expansion.
    pub netmask: T,
    /// The bound socket
    pub socket: std::cell::RefCell<Option<tokio::net::UdpSocket>>
}

/// To keep track of the listen addresses and open sockets
pub struct InterfaceListenAddresses {
    /// The interface that this is referring to
    pub interface_name: String,
    /// The interface index (for sorting/referencing)
    interface_index: usize,
    /// The OS interface index (for OS reference)
    interface_id: u32,

    /// Indicate if IPv4 is enabled for this interface
    pub ipv4_enabled: bool,
    /// A pointer to which interface is recieving multicast
    pub ipv4_multicast_interface_address_index: Option<usize>,
    /// A list of all the bound sockets on this interface
    pub ipv4_sockets: Vec<InterfaceSocket<SocketAddrV4>>,

    /// Indicate if IPv6 is enabled for this interface
    pub ipv6_enabled: bool,
    /// A pointer to which interface is recieving multicast
    pub ipv6_multicast_interface_address_index: Option<usize>,
    /// A list of all the bound sockets on this interface
    pub ipv6_sockets: Vec<InterfaceSocket<SocketAddrV6>>,
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
    if let Some(storage) = maybe_storage {
        if let Some(sin) = storage.as_sockaddr_in() {
            return Some(SocketAddrV4::new(sin.ip(), port).into());
        } else if let Some(sin) = storage.as_sockaddr_in6() {
            return Some(SocketAddrV6::new(sin.ip(), port, sin.flowinfo(), sin.scope_id()).into());
        }
    }
    None
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

/// Get a list of addresses/interfaces o bind to.
/// 
/// There's a complexity in how this works. Since we allow the configuration
/// to specify different interfaces, we must bind to each interface
/// independently. This means we also need to keep track of the source
/// interface only reply to that interface. For an interface to be selected
/// it should have both the interface and an address of the IP version active.
pub async fn get_network_interfaces(net_config: &crate::config::AppNetworkConfiguration) -> Result<Vec<InterfaceListenAddresses>, ConnectorError> {
    let mut interfaces: std::collections::HashMap<String, std::collections::HashMap<AddressFamily, Vec<InterfaceAddress>>> = std::collections::HashMap::new();
    let mut maybe_only_interfaces: Option<std::collections::HashMap<&str, &crate::config::AppInterfaceConfiguration>> = None;
    let mut ilas: Vec<InterfaceListenAddresses> = Vec::new();

    // Create a list of interfaces to filter by
    if let Some(interfaces) = &(net_config.interfaces) {
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
        if net_config.skip_ptp && ifaddr.flags.contains(InterfaceFlags::IFF_POINTOPOINT) {
            log::debug!("Skipping address on point-to-point interface '{}'", interface_name);
            continue;
        }

        // Filter any ifaddr which does not have an address or is loop back.
        if let Some(ipaddr) = match ifaddr.address {
            Some(a) if a.family().is_some_and(|f: AddressFamily| f == AddressFamily::Inet) => Some(IpAddr::V4(a.as_sockaddr_in().unwrap().ip())),
            Some(a) if a.family().is_some_and(|f: AddressFamily| f == AddressFamily::Inet6) => Some(IpAddr::V6(a.as_sockaddr_in6().unwrap().ip())),
            _ => continue
        } {
            if ipaddr.is_loopback() {
                continue;
            }
        }

        // Check if we have an interface filter active.
        if let Some(only_interfaces) = &maybe_only_interfaces {
            if !only_interfaces.contains_key(&*interface_name) {
                continue;
            }
        }

        // Save the interface under the correct family
        let entry = interfaces.entry(interface_name).or_insert_with(|| {
            let mut new_map = std::collections::HashMap::new();
            new_map.insert(AddressFamily::Inet, Vec::new());
            new_map.insert(AddressFamily::Inet6, Vec::new());
            new_map
        });

        // Save the ifaddr
        let iface_addresses = entry.get_mut(&ifaddr.address.unwrap().family().unwrap()).unwrap();
        iface_addresses.push(ifaddr);
        // TODO: migrate away from the intermediate HashMap
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
    // different source address.

    // Sort the interface lists to determine which address to use.
    for (iface_index, iface) in interfaces.iter_mut().enumerate() {
        // Get the correct port
        let port = if let Some(only_interfaces) = &maybe_only_interfaces && only_interfaces.contains_key(iface.0.as_str()) {
            only_interfaces.get(iface.0.as_str()).unwrap().port
        } else {
            DEFAULT_PORT
        };

        // Get the correct interface id. Should always be successful.
        let interface_name = CString::new(iface.0.as_str()).expect("Null values in interface name");
        let os_interface_index = if_nametoindex(interface_name.as_c_str())?;

        let mut ila = InterfaceListenAddresses{
            interface_name: iface.0.clone(),
            interface_index: iface_index * 1000,
            interface_id: os_interface_index,
            
            ipv4_enabled: if let Some(only_interfaces) = &maybe_only_interfaces && only_interfaces.contains_key(iface.0.as_str()) {
                    only_interfaces.get(iface.0.as_str()).unwrap().ipv4
                } else {
                    true
                },
            ipv4_multicast_interface_address_index: None,
            ipv4_sockets: Vec::new(),

            ipv6_enabled: if let Some(only_interfaces) = &maybe_only_interfaces && only_interfaces.contains_key(iface.0.as_str()) {
                    only_interfaces.get(iface.0.as_str()).unwrap().ipv6
                } else {
                    true
                },
            ipv6_multicast_interface_address_index: None,
            ipv6_sockets: Vec::new(),

        };
        
        // Process the IPV4 addresses
        if ila.ipv4_enabled {
            if iface.1.get(&AddressFamily::Inet).unwrap().len() > 0 {
                // IPv4 is enabled, however there still need to be a valid IPv4 address for it to really be enabled.
                for (ifaddr_index, ifaddr) in iface.1.get(&AddressFamily::Inet).unwrap().iter().enumerate() {
                    let ip_address = sockaddr_nix_to_stdnet(ifaddr.address, port).unwrap();
                    let ip_netmask = sockaddr_nix_to_stdnet(ifaddr.netmask, port).unwrap();
                    let ip_network = to_network(ip_address, ip_netmask)?;
                    let is: InterfaceSocket<SocketAddrV4> = InterfaceSocket {
                        interface_address_index: ila.interface_index + ifaddr_index,
                        address: match ip_address { SocketAddr::V4(sin) => sin, _ => unreachable!() },
                        netmask: match ip_netmask { SocketAddr::V4(sin) => sin, _ => unreachable!() },
                        network: match ip_network { SocketAddr::V4(sin) => sin, _ => unreachable!() },
                        socket: std::cell::RefCell::new(None)
                    };

                    ila.ipv4_sockets.push(is);
                }

                // Sort the IPv4 addresses to find a suitable address that supports multicast.
                let mut filtered_addresses: Vec<&InterfaceAddress> = iface.1.get(&AddressFamily::Inet).unwrap().iter()
                    .filter(|&e| e.flags.contains(InterfaceFlags::IFF_MULTICAST))
                    .collect();
                filtered_addresses.sort_by_key(|&e| {
                    let sin = e.address.as_ref().unwrap().as_sockaddr_in().unwrap();
                    let ip = sin.ip();
                    1 * (if ip.is_link_local() {1000} else if ip.is_private() {2000} else if is_globalish4(&ip) {3000} else {5000})
                });

                if let Some(&filtered_addresses_first) = filtered_addresses.get(0) {
                    // Find the address reference
                    let ip_multicast = match sockaddr_nix_to_stdnet(filtered_addresses_first.address, 0).unwrap() { SocketAddr::V4(sin) => sin, _ => unreachable!() };
                    ila.ipv4_multicast_interface_address_index = Some(ila.ipv4_sockets.iter().find(|&e| e.address.ip() == ip_multicast.ip()).unwrap().interface_address_index);
                } else {
                    log::info!("Interface {} IPv4 multicast disabled. No suitable addresses", ila.interface_name);
                }
            } else {
                log::info!("Interface {} IPv4 disabled. No available IPv4 addresses", ila.interface_name);
                ila.ipv4_enabled = false;
            }
        }

        // Repeat for the IPv6 addresses. Note that we prefer a ULA over a global here because we are specifically looking
        // for local traffic. We might choose to listen to all addresses, the the multicast bind address and primary
        // should address should really be local.
        if ila.ipv6_enabled {
            if iface.1.get(&AddressFamily::Inet6).unwrap().len() > 0 {
                // IPv4 is enabled, however there still need to be a valid IPv4 address for it to really be enabled.
                for (ifaddr_index, ifaddr) in iface.1.get(&AddressFamily::Inet6).unwrap().iter().enumerate() {
                    let ip_address = sockaddr_nix_to_stdnet(ifaddr.address, port).unwrap();
                    let ip_netmask = sockaddr_nix_to_stdnet(ifaddr.netmask, port).unwrap();
                    let ip_network = to_network(ip_address, ip_netmask)?;
                    let is: InterfaceSocket<SocketAddrV6> = InterfaceSocket {
                        interface_address_index: ila.interface_index + ifaddr_index,
                        address: match ip_address { SocketAddr::V6(sin) => sin, _ => unreachable!() },
                        netmask: match ip_netmask { SocketAddr::V6(sin) => sin, _ => unreachable!() },
                        network: match ip_network { SocketAddr::V6(sin) => sin, _ => unreachable!() },
                        socket: std::cell::RefCell::new(None)
                    };

                    ila.ipv6_sockets.push(is);
                }

                // Sort the IPv6 addresses to find a suitable address that supports multicast.
                let mut filtered_addresses: Vec<&InterfaceAddress> = iface.1.get(&AddressFamily::Inet6).unwrap().iter()
                    .filter(|&e| e.flags.contains(InterfaceFlags::IFF_MULTICAST))
                    .collect();
                filtered_addresses.sort_by_key(|&e| {
                    let sin = e.address.as_ref().unwrap().as_sockaddr_in6().unwrap();
                    let ip = sin.ip();
                    1 * (if ip.is_unicast_link_local() {1000} else if ip.is_unique_local() {2000} else if is_globalish6(&ip) {3000} else {5000})
                });

                if let Some(&filtered_addresses_first) = filtered_addresses.get(0) {
                    // Find the address reference
                    let ip_multicast = match sockaddr_nix_to_stdnet(filtered_addresses_first.address, 0).unwrap() { SocketAddr::V6(sin) => sin, _ => unreachable!() };
                    ila.ipv6_multicast_interface_address_index = Some(ila.ipv6_sockets.iter().find(|&e| e.address.ip() == ip_multicast.ip()).unwrap().interface_address_index);
                } else {
                    log::info!("Interface {} IPv6 multicast disabled. No suitable addresses", ila.interface_name);
                }
            } else {
                log::info!("Interface {} IPv6 disabled. No available IPv4 addresses", ila.interface_name);
                ila.ipv6_enabled = false;
            }
        }

        // Save the interface info
        ilas.push(ila);
    }

    // Display the networking configuration
    for ila in ilas.iter() {
        log::debug!("Interface '{}'", ila.interface_name);
        if ila.ipv4_enabled {
            let addrs = ila.ipv4_sockets.iter().map(|e|
                format!("{}:{}{}", e.address.ip().to_string(), e.address.port(), if Some(e.interface_address_index) == ila.ipv4_multicast_interface_address_index {"(M)"} else {""})
            ).collect::<Vec<String>>().join(", ");
            log::debug!("  IPv4 addresses: {}", addrs);
        }
        if ila.ipv6_enabled {
            let addrs = ila.ipv6_sockets.iter().map(|e|
                format!("{}:{}{}", e.address.ip().to_string(), e.address.port(), if Some(e.interface_address_index) == ila.ipv6_multicast_interface_address_index {"(M)"} else {""})
            ).collect::<Vec<String>>().join(", ");
            log::debug!("  IPv6 addresses: {}", addrs);
        }
    }

    Ok(ilas)
}

/// Open all of the network connections
pub async fn open_sockets(ilas: &Vec<InterfaceListenAddresses>) -> Result<(), ConnectorError> {
    // Iterate through each and open the sockets
    for ila in ilas.iter() {
        if ila.ipv4_enabled {
            for is in ila.ipv4_sockets.iter() {
                let join_multicast = ila.ipv4_multicast_interface_address_index == Some(is.interface_address_index);
                log::info!("Opening {}:{} on interface {}{}", is.address.ip(), is.address.port(), ila.interface_name, if join_multicast {" with multicast"} else {""});
                let socket = UdpSocket::bind(is.address).await?;

                if join_multicast {
                    socket.join_multicast_v4(MULTICAST_IPV4, is.address.ip().clone())?;
                }
                *is.socket.borrow_mut() = Some(socket);
            }
        }

        if ila.ipv6_enabled {
            for is in ila.ipv6_sockets.iter() {
                let join_multicast = ila.ipv6_multicast_interface_address_index == Some(is.interface_address_index);
                log::info!("Opening {}:{} on interface {}{}", is.address.ip(), is.address.port(), ila.interface_name, if join_multicast {" with multicast"} else {""});
                let socket = UdpSocket::bind(is.address).await?;

                if join_multicast {
                    socket.join_multicast_v6(&MULTICAST_IPV6, ila.interface_id)?;
                }
                *is.socket.borrow_mut() = Some(socket);
            }
        }
    }

    Ok(())
}

/// Close all sockets
pub async fn close_sockets(ilas: &Vec<InterfaceListenAddresses>) -> Result<(), ConnectorError> {
    log::info!("Closing all open sockets");

    // Iterate through each and open the sockets
    for ila in ilas.iter() {
        for is in ila.ipv4_sockets.iter() {
            {
                let maybe_socket = is.socket.borrow();
                if let Some(socket) = maybe_socket.as_ref() && Some(is.interface_address_index) == ila.ipv4_multicast_interface_address_index {
                    log::debug!("leaving IPv4 multicast on interface '{}'", ila.interface_name);
                    socket.leave_multicast_v4(MULTICAST_IPV4, is.address.ip().clone())?;
                }
            }
            *is.socket.borrow_mut() = None;
        }
        for is in ila.ipv6_sockets.iter() {
            {
                let maybe_socket = is.socket.borrow();
                if let Some(socket) = maybe_socket.as_ref() && Some(is.interface_address_index) == ila.ipv6_multicast_interface_address_index {
                    log::debug!("leaving IPv6 multicast on interface '{}'", ila.interface_name);
                    socket.leave_multicast_v6(&MULTICAST_IPV6, ila.interface_id)?;
                }
            }
            *is.socket.borrow_mut() = None;
        }
    }

    Ok(())
}
