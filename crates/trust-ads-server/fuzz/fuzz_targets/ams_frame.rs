#![no_main]

use libfuzzer_sys::fuzz_target;
use trust_ads_server::AmsTcpFrame;

fuzz_target!(|data: &[u8]| {
    let _ = AmsTcpFrame::parse(data, 4096);
});
