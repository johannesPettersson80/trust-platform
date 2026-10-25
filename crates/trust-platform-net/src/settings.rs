//! What a chip's network is built from: its MAC address and the device settings of its interface
//! (specification 34 TARGET-02), which the settings' own checks have passed.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use smoltcp::wire::{Ipv4Address, Ipv4Cidr};
use trust_runtime_services::device_settings::{ipv4, AddressSettings, DeviceSettings};

/// How the interface gets its IPv4 address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Addressing {
    /// From a DHCP server, with its gateway and name servers.
    Dhcp,
    /// Fixed by the device settings.
    Fixed {
        /// The address with its prefix length.
        address: Ipv4Cidr,
        /// The gateway, if any.
        gateway: Option<Ipv4Address>,
        /// Up to two name servers.
        dns: Vec<Ipv4Address>,
    },
}

impl Addressing {
    /// The addressing of an interface's settings: `dhcp`, the default, or the fixed address with
    /// its gateway and name servers. A fixed address the checks would refuse falls back to DHCP.
    #[must_use]
    pub fn of(settings: &AddressSettings) -> Self {
        let fixed = settings
            .address
            .as_deref()
            .filter(|text| *text != "dhcp")
            .and_then(|text| text.split_once('/'))
            .and_then(|(address, prefix)| {
                let address = Ipv4Address::from(ipv4(address)?);
                let prefix = prefix.parse::<u8>().ok().filter(|prefix| *prefix <= 32)?;
                Some(Ipv4Cidr::new(address, prefix))
            });
        match fixed {
            None => Self::Dhcp,
            Some(address) => Self::Fixed {
                address,
                gateway: settings
                    .gateway
                    .as_deref()
                    .and_then(ipv4)
                    .map(Ipv4Address::from),
                dns: settings
                    .dns
                    .iter()
                    .filter_map(|server| ipv4(server).map(Ipv4Address::from))
                    .collect(),
            },
        }
    }
}

/// The settings of a chip's network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSettings {
    /// The interface's MAC address.
    pub mac: [u8; 6],
    /// How the interface gets its address.
    pub addressing: Addressing,
    /// The name the device gives the DHCP server and its discovery: `[device] name`, or `trust-`
    /// and the last six hexadecimal digits of the MAC address.
    pub host_name: String,
    /// The TCP port of the control connection, none while the settings set no control token: a
    /// chip serves its control connection over a network only with a token (ARCH-12).
    pub control_port: Option<u16>,
}

impl NetworkSettings {
    /// The network of the interface `interface` of the device `settings`, with the MAC address
    /// `mac`.
    #[must_use]
    pub fn new(settings: &DeviceSettings, interface: &AddressSettings, mac: [u8; 6]) -> Self {
        let host_name = settings
            .device
            .name
            .clone()
            .unwrap_or_else(|| format!("trust-{:02x}{:02x}{:02x}", mac[3], mac[4], mac[5]));
        Self {
            mac,
            addressing: Addressing::of(interface),
            host_name,
            control_port: settings
                .control
                .token
                .as_ref()
                .map(|_| settings.control.port()),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;
    use alloc::vec;

    use trust_runtime_services::device_settings::ControlSection;

    use super::*;

    const MAC: [u8; 6] = [0x20, 0x6e, 0xf1, 0x17, 0x04, 0xc0];

    fn interface(address: Option<&str>, gateway: Option<&str>, dns: &[&str]) -> AddressSettings {
        AddressSettings {
            address: address.map(ToString::to_string),
            gateway: gateway.map(ToString::to_string),
            dns: dns.iter().map(ToString::to_string).collect(),
        }
    }

    /// `dhcp` and no address mean DHCP; a fixed address keeps its prefix, gateway and name
    /// servers. Catches a fixed address taken as DHCP, which would leave a plant network without a
    /// DHCP server unreachable.
    #[test]
    fn the_address_settings_give_the_addressing() {
        assert_eq!(
            Addressing::of(&interface(None, None, &[])),
            Addressing::Dhcp
        );
        assert_eq!(
            Addressing::of(&interface(Some("dhcp"), None, &[])),
            Addressing::Dhcp
        );
        assert_eq!(
            Addressing::of(&interface(
                Some("192.168.1.50/24"),
                Some("192.168.1.1"),
                &["192.168.1.1", "9.9.9.9"]
            )),
            Addressing::Fixed {
                address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 1, 50), 24),
                gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
                dns: vec![
                    Ipv4Address::new(192, 168, 1, 1),
                    Ipv4Address::new(9, 9, 9, 9)
                ],
            }
        );
    }

    /// The host name is the device's name, or `trust-` and the MAC's last six hexadecimal digits;
    /// the control port is served only with a token. Catches a chip that serves its control
    /// connection on a network without a token, and two chips announcing one name.
    #[test]
    fn the_name_and_the_control_port_follow_the_settings() {
        let mut settings = DeviceSettings::default();
        let dhcp = interface(None, None, &[]);
        let unnamed = NetworkSettings::new(&settings, &dhcp, MAC);
        assert_eq!(unnamed.host_name, "trust-1704c0");
        assert_eq!(unnamed.control_port, None);
        settings.device.name = Some("line-3".to_string());
        settings.control = ControlSection {
            token: Some("sixteen-chars-ab".to_string()),
            port: None,
        };
        let named = NetworkSettings::new(&settings, &dhcp, MAC);
        assert_eq!(named.host_name, "line-3");
        assert_eq!(named.control_port, Some(9901));
    }
}
