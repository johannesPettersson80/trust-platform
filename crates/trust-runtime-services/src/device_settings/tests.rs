use alloc::string::ToString;
use alloc::vec;

use serde_json::json;

use super::*;

/// The ESP32-C6's interfaces (chips/esp32c6.toml): Wi-Fi and Bluetooth, no Ethernet, a 40 MHz crystal.
const C6: DeviceInterfaces = DeviceInterfaces {
    model: "ESP32-C6",
    wifi: true,
    ethernet: false,
    bluetooth: true,
    crystal_hz: (40_000_000, 40_000_000),
};

const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn plant() -> Value {
    json!({
        "device": {"name": "line-3"},
        "network": {"wifi": {"ssid": "Plant", "password": "secret-pass"}},
        "time": {"server": "pool.ntp.org"},
        "signing": {"trusted_keys": [KEY]},
    })
}

fn refusal(value: &Value) -> String {
    DeviceSettings::from_value(value)
        .and_then(|settings| settings.check(&C6))
        .expect_err("refused")
}

/// A C6's settings with a name, a Wi-Fi network over DHCP, a time server and a trusted key are taken.
/// Catches a check that refuses a valid file, which would leave a chip unable to join its network.
#[test]
fn a_chips_settings_are_taken() {
    let settings = DeviceSettings::from_value(&plant()).expect("the settings read");
    assert_eq!(settings.check(&C6), Ok(()));
    assert_eq!(settings.device.name.as_deref(), Some("line-3"));
    assert_eq!(
        settings.trusted_keys(),
        vec![[
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB,
            0xCC, 0xDD, 0xEE, 0xFF
        ]]
    );
    let fixed = json!({"network": {"wifi": {"ssid": "Plant", "address": "192.168.1.50/24",
        "gateway": "192.168.1.1", "dns": ["192.168.1.1", "9.9.9.9"]}}});
    let fixed = DeviceSettings::from_value(&fixed).expect("the settings read");
    assert_eq!(fixed.check(&C6), Ok(()));
}

/// Each rule refuses with its key and reason (TARGET-02; ARCH-13 for hardware the chip lacks).
/// Catches a setting the chip cannot carry out stored, which it would only fail on at the next boot.
#[test]
fn a_setting_the_device_cannot_take_is_refused_with_its_reason() {
    let with = |pointer: &str, value: Value| {
        let mut settings = plant();
        *settings.pointer_mut(pointer).expect("the key") = value;
        refusal(&settings)
    };
    assert_eq!(
        with("/device/name", json!("-line")),
        "device.name must be 1 to 32 letters, digits and hyphens, not beginning or ending with a hyphen"
    );
    assert_eq!(
        with("/network/wifi/ssid", json!("")),
        "network.wifi.ssid must be 1 to 32 bytes"
    );
    assert_eq!(
        with("/network/wifi/password", json!("short12")),
        "network.wifi.password must be empty, 8 to 63 printable characters, or 64 hexadecimal digits"
    );
    assert_eq!(
        refusal(&json!({"network": {"wifi": {"ssid": "Plant", "country": "se"}}})),
        "network.wifi.country must be two capital letters, an ISO 3166-1 code such as SE"
    );
    assert_eq!(
        with("/time/server", json!("ntp..example")),
        "time.server must be a host name or an IPv4 address"
    );
    assert_eq!(
        with("/signing/trusted_keys", json!([&KEY[1..]])),
        "signing.trusted_keys: '".to_string() + &KEY[1..] + "' is not 64 hexadecimal digits"
    );
    assert_eq!(
        refusal(&json!({"network": {"wifi": {"ssid": "Plant", "address": "192.168.1.300/24"}}})),
        "network.wifi.address must be \"dhcp\" or an IPv4 address with its prefix length, such as \
         192.168.1.50/24"
    );
    assert_eq!(
        refusal(&json!({"network": {"wifi": {"ssid": "Plant", "gateway": "192.168.1.1"}}})),
        "network.wifi: a gateway and name servers are given with a fixed address; DHCP gives them \
         otherwise"
    );
    assert_eq!(
        refusal(&json!({"network": {"ethernet": {}}})),
        "network.ethernet: the ESP32-C6 has no Ethernet"
    );
    assert_eq!(
        refusal(&json!({"hardware": {"crystal_hz": 26_000_000}})),
        "hardware.crystal_hz: the ESP32-C6 takes a crystal of 40000000 to 40000000 Hz"
    );
    assert!(refusal(&json!({"device": {"colour": "blue"}})).contains("unknown field `colour`"));
    let token = "control.token must be 16 to 128 printable ASCII characters without spaces";
    assert_eq!(
        refusal(&json!({"control": {"token": "fifteen-chars-x"}})),
        token
    );
    assert_eq!(
        refusal(&json!({"control": {"token": "sixteen chars ab"}})),
        token
    );
    assert_eq!(
        refusal(&json!({"control": {"token": "x".repeat(129)}})),
        token
    );
    assert_eq!(
        refusal(&json!({"control": {"port": 0}})),
        "control.port must be 1 to 65535"
    );
}

/// A control token of 16 and of 128 printable characters is taken, and the port defaults to 9901.
/// Catches a token check off by one at either end, and a network control connection on no port.
#[test]
fn a_control_token_and_port_are_taken() {
    let settings = DeviceSettings::from_value(&json!({
        "network": {"wifi": {"ssid": "Plant"}},
        "control": {"token": "sixteen-chars-ab"},
    }))
    .expect("the settings read");
    assert_eq!(settings.check(&C6), Ok(()));
    assert_eq!(settings.control.port(), 9901);
    assert!(settings.network.names_one());
    let long = DeviceSettings::from_value(&json!({
        "control": {"token": "y".repeat(128), "port": 5050},
    }))
    .expect("the settings read");
    assert_eq!(long.check(&C6), Ok(()));
    assert_eq!(long.control.port(), 5050);
    assert!(!long.network.names_one());
}

/// `device.get` reports every key but the Wi-Fi password, and whether one is set. Catches a password
/// sent back over the link, where any viewer of the reply would read it.
#[test]
fn the_report_leaves_the_password_out() {
    let settings = DeviceSettings::from_value(&plant()).expect("the settings read");
    let reported = settings.reported();
    assert_eq!(
        reported["network"]["wifi"],
        json!({"ssid": "Plant", "password_set": true})
    );
    assert!(!reported.to_string().contains("secret-pass"));
    let open = DeviceSettings::from_value(&json!({"network": {"wifi": {"ssid": "Guest"}}}))
        .expect("the settings read");
    assert_eq!(
        open.reported()["network"]["wifi"]["password_set"],
        json!(false)
    );
}

/// `device.get` reports the control port and whether a token is set, never the token. Catches the
/// token that guards the chip's network sent back to any client that reads the settings.
#[test]
fn the_report_leaves_the_control_token_out() {
    let settings = DeviceSettings::from_value(&json!({
        "control": {"token": "a-token-of-twenty-chars", "port": 9902},
    }))
    .expect("the settings read");
    let reported = settings.reported();
    assert_eq!(
        reported["control"],
        json!({"port": 9902, "token_set": true})
    );
    assert!(!reported.to_string().contains("a-token-of-twenty-chars"));
    assert_eq!(
        DeviceSettings::default().reported()["control"],
        json!({"token_set": false})
    );
}

/// A record holds the magic, the sequence, the length and the CRC-32 of its text; the newer of two
/// sectors wins, an erased or half-written sector holds none, and settings longer than a sector are
/// not encoded. Catches a write that power ended taken as the settings, and the older record winning.
#[test]
fn the_newer_record_of_two_sectors_holds_the_settings() {
    let settings = DeviceSettings::from_value(&plant()).expect("the settings read");
    let mut first = encode(&settings, 3, 4_096).expect("the record fits");
    assert_eq!(&first[..8], b"TRSTDS01");
    assert_eq!(first[8..12], 3u32.to_le_bytes());
    let text = serde_json::to_vec(&settings).expect("the text");
    assert_eq!(
        first[12..16],
        u32::try_from(text.len()).expect("short").to_le_bytes()
    );
    assert_eq!(decode(&first), Some((3, settings.clone())));
    let second = encode(&DeviceSettings::default(), 4, 4_096).expect("the record fits");
    let erased = vec![0xFF; 4_096];
    assert_eq!(
        newest([&first, &second]).map(|(index, sequence, _)| (index, sequence)),
        Some((1, 4))
    );
    assert_eq!(
        newest([&erased, &first]).map(|(index, sequence, _)| (index, sequence)),
        Some((1, 3))
    );
    assert_eq!(newest([&erased, &erased]), None);
    let last = first.len() - 1;
    first[last] ^= 0x01;
    assert_eq!(decode(&first), None, "a damaged text is no record");
    assert_eq!(encode(&settings, 5, 40), None);
}
