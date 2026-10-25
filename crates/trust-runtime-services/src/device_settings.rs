//! A device's own settings, `device.toml` (specification 34 TARGET-02; specification 11, the device
//! settings contract): the settings that the operating system holds on a computer, kept on a chip that
//! has none. The device name, its network interfaces with DHCP or a fixed address, the Wi-Fi network
//! and its password, Bluetooth, the time server, the keys whose signed programs it trusts, and the few
//! wiring values the firmware cannot detect.
//!
//! The tools read `device.toml` on the computer and send it as a JSON object of the same shape
//! (`device.set`); the chip checks it against its own interfaces ([`DeviceInterfaces`]) and keeps it
//! in the two sectors of its chip file's `device_a` and `device_b`, the newer record winning, so that
//! a write that power ends leaves the settings before it ([`encode`], [`newest`]). `device.get` reports
//! them without the Wi-Fi password ([`DeviceSettings::reported`]). Nothing here reads a flash or a
//! link.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trust_runtime_core::retain::checkpoint::FlashError;

/// The magic word of a settings record.
pub const MAGIC: [u8; 8] = *b"TRSTDS01";
/// The bytes before a record's text: the magic, the sequence, the length and the CRC-32 of the text.
pub const RECORD_HEADER_BYTES: usize = 20;
/// The most trusted keys a device keeps.
pub const MAX_TRUSTED_KEYS: usize = 8;

/// A device's settings, as `device.toml` holds them. Every section and key may be left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSettings {
    /// `[device]`.
    #[serde(default)]
    pub device: DeviceSection,
    /// `[network]`.
    #[serde(default)]
    pub network: NetworkSection,
    /// `[bluetooth]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bluetooth: Option<BluetoothSection>,
    /// `[time]`.
    #[serde(default)]
    pub time: TimeSection,
    /// `[signing]`.
    #[serde(default)]
    pub signing: SigningSection,
    /// `[hardware]`.
    #[serde(default)]
    pub hardware: HardwareSection,
}

/// `[device]`: the device's name, which its network and Bluetooth announce.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSection {
    /// The name: 1 to 32 letters, digits and hyphens, neither first nor last a hyphen (a DNS label).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `[network]`: the device's network interfaces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkSection {
    /// `[network.wifi]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi: Option<WifiSettings>,
    /// `[network.ethernet]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethernet: Option<AddressSettings>,
}

/// `[network.wifi]`: the network the device joins as a station, and its address.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WifiSettings {
    /// The network's name: 1 to 32 bytes.
    pub ssid: String,
    /// The WPA2 passphrase, 8 to 63 printable ASCII characters, or 64 hexadecimal digits; empty for an
    /// open network.
    #[serde(default)]
    pub password: String,
    /// `dhcp`, the default, or a fixed IPv4 address with its prefix length, `192.168.1.50/24`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The gateway of a fixed address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// Up to two name servers of a fixed address.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,
    /// The country whose radio rules apply, as its ISO 3166-1 alpha-2 code (`SE`, `DE`, `US`);
    /// without one, the channels and power allowed everywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

impl WifiSettings {
    /// The interface's address settings.
    #[must_use]
    pub fn address_settings(&self) -> AddressSettings {
        AddressSettings {
            address: self.address.clone(),
            gateway: self.gateway.clone(),
            dns: self.dns.clone(),
        }
    }
}

/// The address of an interface: from DHCP, or fixed with its gateway and name servers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddressSettings {
    /// `dhcp`, the default, or a fixed IPv4 address with its prefix length, `192.168.1.50/24`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The gateway of a fixed address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// Up to two name servers of a fixed address.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,
}

/// `[bluetooth]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BluetoothSection {
    /// Whether the device advertises itself over Bluetooth.
    #[serde(default)]
    pub enabled: bool,
}

/// `[time]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSection {
    /// The time server (SNTP), a host name or an IPv4 address (TARGET-06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
}

/// `[signing]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningSection {
    /// The Ed25519 public keys whose signed programs the device admits, each 64 hexadecimal digits
    /// (specification 44 §5.11).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_keys: Vec<String>,
}

/// `[hardware]`: the wiring values the firmware cannot detect.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareSection {
    /// The crystal's frequency in Hz, where the firmware cannot measure it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crystal_hz: Option<u32>,
    /// The Ethernet PHY, where the firmware cannot identify it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethernet_phy: Option<String>,
}

/// The interfaces a device has, which its settings are checked against (ARCH-13: a setting the
/// device has no hardware for is refused with the reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceInterfaces {
    /// The chip's model, which a refusal names.
    pub model: &'static str,
    /// Whether it has Wi-Fi.
    pub wifi: bool,
    /// Whether it has Ethernet.
    pub ethernet: bool,
    /// Whether it has Bluetooth.
    pub bluetooth: bool,
    /// The crystal frequencies it takes, in Hz.
    pub crystal_hz: (u32, u32),
}

impl DeviceSettings {
    /// Reads the settings of a `device.set` request.
    ///
    /// # Errors
    ///
    /// The serde error's text: an unknown section or key, or a value of the wrong type.
    pub fn from_value(value: &Value) -> Result<Self, String> {
        Self::deserialize(value).map_err(|error| error.to_string())
    }

    /// Checks the settings against the device's `interfaces`.
    ///
    /// # Errors
    ///
    /// The first refusal, which names the key and the reason.
    pub fn check(&self, interfaces: &DeviceInterfaces) -> Result<(), String> {
        if let Some(name) = &self.device.name {
            check_name("device.name", name, 32)?;
        }
        if let Some(wifi) = &self.network.wifi {
            if !interfaces.wifi {
                return Err(format!(
                    "network.wifi: the {} has no Wi-Fi",
                    interfaces.model
                ));
            }
            if wifi.ssid.is_empty() || wifi.ssid.len() > 32 {
                return Err("network.wifi.ssid must be 1 to 32 bytes".into());
            }
            check_password(&wifi.password)?;
            check_address("network.wifi", &wifi.address_settings())?;
            if let Some(country) = &wifi.country
                && !(country.len() == 2 && country.bytes().all(|byte| byte.is_ascii_uppercase()))
            {
                return Err(
                    "network.wifi.country must be two capital letters, an ISO 3166-1 code such as SE"
                        .into(),
                );
            }
        }
        if let Some(ethernet) = &self.network.ethernet {
            if !interfaces.ethernet {
                return Err(format!(
                    "network.ethernet: the {} has no Ethernet",
                    interfaces.model
                ));
            }
            check_address("network.ethernet", ethernet)?;
        }
        if self
            .bluetooth
            .as_ref()
            .is_some_and(|bluetooth| bluetooth.enabled)
            && !interfaces.bluetooth
        {
            return Err(format!(
                "bluetooth: the {} has no Bluetooth",
                interfaces.model
            ));
        }
        if let Some(server) = &self.time.server
            && ipv4(server).is_none()
        {
            check_host("time.server", server)?;
        }
        if self.signing.trusted_keys.len() > MAX_TRUSTED_KEYS {
            return Err(format!(
                "signing.trusted_keys: at most {MAX_TRUSTED_KEYS} keys"
            ));
        }
        for key in &self.signing.trusted_keys {
            if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!(
                    "signing.trusted_keys: '{key}' is not 64 hexadecimal digits"
                ));
            }
        }
        if let Some(crystal) = self.hardware.crystal_hz {
            let (low, high) = interfaces.crystal_hz;
            if crystal < low || crystal > high {
                return Err(format!(
                    "hardware.crystal_hz: the {} takes a crystal of {low} to {high} Hz",
                    interfaces.model
                ));
            }
        }
        if self.hardware.ethernet_phy.is_some() && !interfaces.ethernet {
            return Err(format!(
                "hardware.ethernet_phy: the {} has no Ethernet",
                interfaces.model
            ));
        }
        Ok(())
    }

    /// The settings as `device.get` reports them: every key but the Wi-Fi password, whose place
    /// says only whether one is set.
    #[must_use]
    pub fn reported(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(wifi) = value
            .pointer_mut("/network/wifi")
            .and_then(Value::as_object_mut)
        {
            let set = wifi
                .remove("password")
                .and_then(|password| password.as_str().map(|text| !text.is_empty()))
                .unwrap_or(false);
            wifi.insert("password_set".into(), json!(set));
        }
        value
    }

    /// The trusted keys as bytes; the check has made sure each is 64 hexadecimal digits.
    #[must_use]
    pub fn trusted_keys(&self) -> Vec<[u8; 32]> {
        self.signing
            .trusted_keys
            .iter()
            .filter_map(|key| {
                let mut bytes = [0u8; 32];
                for (index, pair) in key.as_bytes().chunks(2).enumerate() {
                    let text = core::str::from_utf8(pair).ok()?;
                    *bytes.get_mut(index)? = u8::from_str_radix(text, 16).ok()?;
                }
                Some(bytes)
            })
            .collect()
    }
}

/// A DNS label of at most `max` letters, digits and hyphens, neither first nor last a hyphen.
fn check_name(key: &str, name: &str, max: usize) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.len() <= max
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{key} must be 1 to {max} letters, digits and hyphens, not beginning or ending with a hyphen"
        ))
    }
}

/// A host name: dot-separated DNS labels, at most 253 characters.
fn check_host(key: &str, host: &str) -> Result<(), String> {
    let valid = !host.is_empty()
        && host.len() <= 253
        && host
            .split('.')
            .all(|label| check_name(key, label, 63).is_ok());
    if valid {
        Ok(())
    } else {
        Err(format!("{key} must be a host name or an IPv4 address"))
    }
}

/// An empty password, a WPA2 passphrase of 8 to 63 printable ASCII characters, or a 64-digit PSK.
fn check_password(password: &str) -> Result<(), String> {
    let passphrase = (8..=63).contains(&password.len())
        && password.bytes().all(|byte| (0x20..=0x7E).contains(&byte));
    let key = password.len() == 64 && password.bytes().all(|byte| byte.is_ascii_hexdigit());
    if password.is_empty() || passphrase || key {
        Ok(())
    } else {
        Err("network.wifi.password must be empty, 8 to 63 printable characters, or 64 hexadecimal digits".into())
    }
}

/// `dhcp` or a fixed address with its prefix length; a gateway and name servers only with a fixed
/// address, which DHCP gives otherwise.
fn check_address(key: &str, settings: &AddressSettings) -> Result<(), String> {
    let fixed = match settings.address.as_deref() {
        None | Some("dhcp") => false,
        Some(text) => {
            let valid = text.split_once('/').is_some_and(|(address, prefix)| {
                ipv4(address).is_some()
                    && prefix
                        .parse::<u8>()
                        .is_ok_and(|length| (1..=32).contains(&length))
            });
            if !valid {
                return Err(format!(
                    "{key}.address must be \"dhcp\" or an IPv4 address with its prefix length, \
                     such as 192.168.1.50/24"
                ));
            }
            true
        }
    };
    if !fixed && (settings.gateway.is_some() || !settings.dns.is_empty()) {
        return Err(format!(
            "{key}: a gateway and name servers are given with a fixed address; DHCP gives them otherwise"
        ));
    }
    if let Some(gateway) = &settings.gateway
        && ipv4(gateway).is_none()
    {
        return Err(format!("{key}.gateway must be an IPv4 address"));
    }
    if settings.dns.len() > 2 || settings.dns.iter().any(|server| ipv4(server).is_none()) {
        return Err(format!("{key}.dns must be at most two IPv4 addresses"));
    }
    Ok(())
}

/// The four bytes of a dotted IPv4 address, `None` for any other text.
#[must_use]
pub fn ipv4(text: &str) -> Option<[u8; 4]> {
    let mut bytes = [0u8; 4];
    let mut parts = text.split('.');
    for byte in &mut bytes {
        let part = parts.next()?;
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|digit| digit.is_ascii_digit()) {
            return None;
        }
        *byte = part.parse().ok()?;
    }
    parts.next().is_none().then_some(bytes)
}

/// The record that holds `settings` as sequence `sequence`: the magic, the sequence, the text's length
/// and CRC-32, and the JSON text. `None` when it does not fit `capacity` bytes, a sector.
#[must_use]
pub fn encode(settings: &DeviceSettings, sequence: u32, capacity: usize) -> Option<Vec<u8>> {
    let text = serde_json::to_vec(settings).ok()?;
    let length = u32::try_from(text.len()).ok()?;
    if RECORD_HEADER_BYTES + text.len() > capacity {
        return None;
    }
    let mut record = Vec::with_capacity(RECORD_HEADER_BYTES + text.len());
    record.extend_from_slice(&MAGIC);
    record.extend_from_slice(&sequence.to_le_bytes());
    record.extend_from_slice(&length.to_le_bytes());
    record.extend_from_slice(&trust_runtime_core::crc32::checksum(&text).to_le_bytes());
    record.extend_from_slice(&text);
    Some(record)
}

/// The settings and sequence of the record at the start of `sector`, `None` for an erased sector or
/// a record that a write did not finish (its CRC-32 is wrong).
#[must_use]
pub fn decode(sector: &[u8]) -> Option<(u32, DeviceSettings)> {
    let header = sector.get(..RECORD_HEADER_BYTES)?;
    if header[..8] != MAGIC {
        return None;
    }
    let word = |at: usize| {
        u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    let length = usize::try_from(word(12)).ok()?;
    let text = sector.get(RECORD_HEADER_BYTES..RECORD_HEADER_BYTES.checked_add(length)?)?;
    if trust_runtime_core::crc32::checksum(text) != word(16) {
        return None;
    }
    let settings = serde_json::from_slice(text).ok()?;
    Some((word(8), settings))
}

/// The newer of the two sectors' records, by its index (0 or 1) and with its sequence; `None` when
/// neither holds one.
#[must_use]
pub fn newest(sectors: [&[u8]; 2]) -> Option<(usize, u32, DeviceSettings)> {
    let read = |index: usize| {
        decode(sectors[index]).map(|(sequence, settings)| (index, sequence, settings))
    };
    match (read(0), read(1)) {
        (Some(first), Some(second)) => Some(if second.1 > first.1 { second } else { first }),
        (first, second) => first.or(second),
    }
}

/// The text of a refusal for settings that do not fit one sector.
#[must_use]
pub fn too_long(capacity: usize) -> String {
    format!(
        "the settings do not fit the device's {} bytes",
        capacity.saturating_sub(RECORD_HEADER_BYTES)
    )
}

/// The flash of a device's two settings sectors (`device_a` and `device_b` of its chip file).
pub trait SettingsFlash {
    /// The bytes of each sector, one erase unit.
    fn sector_bytes(&self) -> usize;

    /// Reads `bytes.len()` bytes from the start of `sector` (0 or 1).
    ///
    /// # Errors
    ///
    /// The flash's failure.
    fn read(&mut self, sector: usize, bytes: &mut [u8]) -> Result<(), FlashError>;

    /// Erases `sector`: every byte becomes 0xFF.
    ///
    /// # Errors
    ///
    /// The flash's failure.
    fn erase(&mut self, sector: usize) -> Result<(), FlashError>;

    /// Programs `bytes`, whole 4-byte units, from the start of `sector`.
    ///
    /// # Errors
    ///
    /// The flash's failure.
    fn program(&mut self, sector: usize, bytes: &[u8]) -> Result<(), FlashError>;
}

/// A device's settings store: its two sectors and the newest record they hold.
pub struct SettingsStore {
    flash: alloc::boxed::Box<dyn SettingsFlash>,
    newest: Option<(usize, u32, DeviceSettings)>,
}

impl SettingsStore {
    /// The store over `flash`, with the newest record read from both sectors; a sector that cannot
    /// be read holds none.
    #[must_use]
    pub fn open(mut flash: alloc::boxed::Box<dyn SettingsFlash>) -> Self {
        let size = flash.sector_bytes();
        let mut first = alloc::vec![0u8; size];
        let mut second = alloc::vec![0u8; size];
        if flash.read(0, &mut first).is_err() {
            first.fill(0xFF);
        }
        if flash.read(1, &mut second).is_err() {
            second.fill(0xFF);
        }
        let newest = newest([&first, &second]);
        Self { flash, newest }
    }

    /// The newest settings with their sequence.
    #[must_use]
    pub fn settings(&self) -> Option<(u32, DeviceSettings)> {
        self.newest
            .as_ref()
            .map(|(_, sequence, settings)| (*sequence, settings.clone()))
    }

    /// Writes `settings` into the sector that does not hold the newest record, with the next
    /// sequence, and reads it back; the record before it stays whole until this one is, so a write
    /// that power ends leaves the settings before it. Returns the sequence.
    ///
    /// # Errors
    ///
    /// Settings longer than a sector, and a flash failure or a read-back that differs.
    pub fn store(&mut self, settings: &DeviceSettings) -> Result<u32, String> {
        let size = self.flash.sector_bytes();
        let (sector, sequence) = self.newest.as_ref().map_or((0, 1), |(index, sequence, _)| {
            (1 - index, sequence.wrapping_add(1))
        });
        let mut record = encode(settings, sequence, size).ok_or_else(|| too_long(size))?;
        record.resize(record.len().next_multiple_of(4).min(size), 0xFF);
        self.flash
            .erase(sector)
            .map_err(|_| String::from("the device settings' flash erase failed"))?;
        self.flash
            .program(sector, &record)
            .map_err(|_| String::from("the device settings' flash program failed"))?;
        let mut read = alloc::vec![0u8; record.len()];
        self.flash
            .read(sector, &mut read)
            .map_err(|_| String::from("the device settings' flash read failed"))?;
        if read != record {
            return Err("the programmed bytes do not read back as written".into());
        }
        self.newest = Some((sector, sequence, settings.clone()));
        Ok(sequence)
    }
}

impl core::fmt::Display for DeviceSettings {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.reported().to_string())
    }
}

#[cfg(test)]
mod tests;
