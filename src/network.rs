use log::{info, warn};
use rand::Rng;
use std::fs;
use std::net::Ipv4Addr;

pub fn load_subnets() -> Vec<(Ipv4Addr, u8)> {
    let content = match fs::read_to_string("assets/ips.txt") {
        Ok(content) => content,
        Err(e) => {
            warn!(
                "Could not read assets/ips.txt: {}, falling back to random IPs",
                e
            );
            return Vec::new();
        }
    };

    let mut subnets = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((ip_str, prefix_str)) = line.split_once('/') {
            if let (Ok(ip), Ok(prefix)) = (ip_str.parse::<Ipv4Addr>(), prefix_str.parse::<u8>()) {
                if prefix <= 32 {
                    subnets.push((ip, prefix));
                }
            }
        }
    }

    info!("Loaded {} subnets from assets/ips.txt", subnets.len());
    subnets
}

/// Generate a random IP address within a given subnet
pub fn random_ip_from_subnet(network: Ipv4Addr, prefix_len: u8) -> Ipv4Addr {
    let mut rng = rand::rng();
    let network_u32 = u32::from_be_bytes(network.octets());
    let host_bits = 32 - prefix_len;
    let max_hosts = if host_bits >= 32 {
        u32::MAX
    } else {
        (1u32 << host_bits) - 1
    };

    let host_offset = if max_hosts <= 2 {
        1
    } else {
        rng.random_range(1..max_hosts)
    };

    let ip_u32 = network_u32 | host_offset;
    Ipv4Addr::from(ip_u32.to_be_bytes())
}

/// Select a random IP address from the provided subnet list
pub fn random_ipv4_from_subnets(subnets: &[(Ipv4Addr, u8)]) -> Ipv4Addr {
    if subnets.is_empty() {
        return random_ipv4_fallback();
    }

    let mut rng = rand::rng();
    let (network, prefix) = subnets[rng.random_range(0..subnets.len())];
    random_ip_from_subnet(network, prefix)
}

/// Fallback random IP generator for when no subnets are available
pub fn random_ipv4_fallback() -> Ipv4Addr {
    let mut rng = rand::rng();
    loop {
        let ip = Ipv4Addr::new(
            rng.random_range(1..=223),
            rng.random_range(0..=255),
            rng.random_range(0..=255),
            rng.random_range(1..=254),
        );

        if ip.is_private() || ip.is_loopback() || ip.octets()[0] == 0 {
            continue;
        }

        return ip;
    }
}

pub fn increment_ip(base: &Ipv4Addr, offset: u32) -> Ipv4Addr {
    let ip_u32 = u32::from_be_bytes(base.octets());
    Ipv4Addr::from((ip_u32.wrapping_add(offset)).to_be_bytes())
}
