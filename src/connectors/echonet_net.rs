//! Network related functions and structs
use nix::{self, ifaddrs::InterfaceAddress, sys::socket::{AddressFamily, SockaddrLike}};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::{error, fmt, io};
use tokio::net::UdpSocket;
use crate::echonet;

/// Constants
const MULTICAST_IPV4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 23, 0), 3610);
const MULTICAST_IPV6: SocketAddrV6 = SocketAddrV6::new(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1), 3610, 0, 2);

/// Network configuration error
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct NetworkConfigurationError {
    message: String
}

// Error traits
impl fmt::Display for NetworkConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Network confogiration error: {}", self.message)
    }
}

impl error::Error for NetworkConfigurationError {}

pub struct NetworkInterfaceXXX {
    // For IPv4, want the host specific address, broadcats and multicast (if multicast routes to this interface)
    // For IPv6, want the host and the all-hosts multicast.
    /*
    pub host: String,
    pub port: u16,
    pub useMdns: bool,
    pub credentials: AppMqttCredentialsConfiguration,
    pub certificate: AppMqttCertificateConfiguration,
    pub truststore: AppMqttCertificateConfiguration,
    */
}

/// Create a list of network interfaces to bind to.
/// 
/// There's a complexity in how this works. Since we allow the configuration
/// to specify different interfaces, we must bind to each interface
/// independently. This means we also need to keep track of the source
/// interface only reply to that interface. For an interface to be selected
/// it should have both the interface and an address of the IP version active.
pub async fn get_network_interfaces() -> Result<(), NetworkConfigurationError> {
    let mut loopback_interface_name: Option<String> = None;
    let mut ifaddrs: Vec<InterfaceAddress> = Vec::new(); // Take ownership of the ones we want, drop the rest.

    // Scan through to select the appropriate interfaces. Need to do this as
    // the loopback interface can have other addresses. This does a lookup of
    // the name so it can be excluded later.
    for ifaddr in nix::ifaddrs::getifaddrs().unwrap() {
        if ifaddr.address.is_none() {
            continue;
        }

        // Determine which addresses/interfaces to use. 
        if let Some(ipaddr) = match ifaddr.address {
            Some(a) if a.family().is_some_and(|f: AddressFamily| f == AddressFamily::Inet) => Some(IpAddr::V4(a.as_sockaddr_in().unwrap().ip())),
            Some(a) if a.family().is_some_and(|f: AddressFamily| f == AddressFamily::Inet6) => Some(IpAddr::V6(a.as_sockaddr_in6().unwrap().ip())),
            _ => None
        } {
            if !ipaddr.is_loopback() {
                println!("Pushing '{}', address {}", ifaddr.interface_name, ifaddr.address.unwrap());
                ifaddrs.push(ifaddr);
            } else {
                loopback_interface_name = Some(ifaddr.interface_name);
            }
        }
    }

    if loopback_interface_name.is_none() {
        return Err(NetworkConfigurationError{ message: String::from("Unable to identify loopback interface") });
    }

    // Create a new Vec of the addresses to bind to. For IPv6, the address will
    // also include the interface.
    let iter = ifaddrs
        .iter()
        .filter(|ifaddr| *loopback_interface_name.as_ref().unwrap() != ifaddr.interface_name)
        .map(|f| match f.address {
            Some(a) if a.family().is_some_and(|f: AddressFamily| f == AddressFamily::Inet) => Some(IpAddr::V4(a.as_sockaddr_in().unwrap().ip())),
            Some(a) if a.family().is_some_and(|f: AddressFamily| f == AddressFamily::Inet6) => Some(IpAddr::V6(a.as_sockaddr_in6().unwrap().ip())),
            _ => None
        })
        .filter(|ipaddr| ipaddr.is_some())
        .map(|ipaddr| ipaddr.unwrap());

    for ipaddr in iter {
        println!("address {}",
               ipaddr); // bugger, this appears to be dropping the %xxx part.
            }

    Ok(())
    //ipaddrs.retain(|ipaddr| ipaddr.);

    // TODO: how to detect the correct IPv4 multicast interface? Route table?
}

/// Run the main listen loop on the network
/// 
/// Communication to the main thread uses two channels. A broadcast channel
/// that recieves messages designed to be broadcast.
pub async fn do_listen() -> io::Result<()> {
// Need to turn interface name into id. Bind to primary IPv4 address (q: ipv4 with multiple addresses?) or link local IPv6 address.

    let mut buf = vec![0u8; 1500];
    let bind_addr = "[::0]:3610".parse::<SocketAddrV6>().unwrap();
    println!("Got here");
    
    let socket = UdpSocket::bind(bind_addr).await?;

    // Need to get the correct interface id. Should be the same as the bind address.
    socket.join_multicast_v6(&MULTICAST_IPV6.ip(), 0)?;

    // Send an EchoNet discovery
    // This is temp for testing, will require a restructure.
    let packet = echonet::wire::EchonetFrame::create_all_nodes_query();
    println!("Got packet: {:#?}", packet);
    // For network 


    // Leave the multicast group
    socket.leave_multicast_v6(&MULTICAST_IPV6.ip(), 0)?;
    
    Ok(())
}
/*
https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/Echonet_lite_old/Echonet_lite_V1_11_en/ECHONET-Lite_Ver.1.11(02)_E.pdf
https://www.google.com/search?q=rust+tokio+communication+between+async+functions&oq=rust+tokio+communication+between+async+functions&gs_lcrp=EgZjaHJvbWUyCQgAEEUYORigATIHCAEQIRifBdIBCTExMzg4ajBqN6gCALACAA&sourceid=chrome&ie=UTF-8
https://www.google.com/search?q=rust+tokio+listen+to+multicast&oq=rust+tokio+listen+to+multicast&gs_lcrp=EgZjaHJvbWUyBggAEEUYOTIHCAEQIRigATIHCAIQIRifBdIBCTExOTQ4ajBqOagCBrACAfEFgbhKuCxTVS8&sourceid=chrome&ie=UTF-8
*/