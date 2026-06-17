#![no_main]

use libfuzzer_sys::fuzz_target;
use trust_ads_server::{
    ams_net_id_bytes_to_text, ams_net_id_text_to_bytes, build_device_notification_payload,
    NotificationSample, NotificationStamp,
};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = ams_net_id_text_to_bytes(text);
    }

    if data.len() >= 6 {
        let mut bytes = [0_u8; 6];
        bytes.copy_from_slice(&data[..6]);
        let text = ams_net_id_bytes_to_text(bytes);
        let _ = ams_net_id_text_to_bytes(&text);
    }

    let mut samples = Vec::new();
    for (index, chunk) in data.chunks(8).take(8).enumerate() {
        samples.push(NotificationSample::new(
            u32::try_from(index).unwrap_or(0).saturating_add(1),
            chunk.to_vec(),
        ));
    }
    let stamp = NotificationStamp::new(0, samples);
    let _ = build_device_notification_payload(&[stamp]);
});
