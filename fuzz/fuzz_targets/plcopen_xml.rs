#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_XML_BYTES: usize = 64 * 1024;

fuzz_target!(|data: &[u8]| {
    let root = std::env::temp_dir().join(format!("trust-plcopen-fuzz-{}", std::process::id()));
    let input = root.join("input.xml");
    let project = root.join("project");
    let _ = std::fs::remove_dir_all(&root);
    if std::fs::create_dir_all(&root).is_err() {
        return;
    }
    if std::fs::write(&input, &data[..data.len().min(MAX_XML_BYTES)]).is_ok() {
        let _ = trust_plcopen::import_xml_to_project(&input, &project);
    }
    let _ = std::fs::remove_dir_all(root);
});
