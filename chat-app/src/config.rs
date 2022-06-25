use std::net::{IpAddr, Ipv4Addr};

pub const SERVER_DEFAULT_IP_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
pub const SERVER_DEFAULT_PORT: u16 = 4200;
