use serde_json::json;

use super::*;

/// A sink that keeps every candidate and warning it is handed.
#[derive(Default)]
struct Recorded {
    candidates: Vec<DiscoverCandidate>,
    warnings: Vec<String>,
}

impl CandidateSink for Recorded {
    fn found(&mut self, candidate: DiscoverCandidate) {
        self.candidates.push(candidate);
    }

    fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// A line the kernel lists through a GPIO character device.
fn listed_line() -> GpioLine {
    GpioLine {
        chip: "gpiochip0".to_string(),
        label: Some("pinctrl-bcm2835".to_string()),
        line: 17,
        name: Some("GPIO17".to_string()),
        consumer: Some("led".to_string()),
        used: true,
        direction: Some("out".to_string()),
        from_sysfs: false,
    }
}

#[test]
fn gpio_source_reports_chip_offset_name_consumer_and_use() {
    // Specification 11, "Sources" and "Identity": a GPIO line is listed by its chip, the chip's
    // label, its offset within the chip, its name, its consumer, whether it is in use, and its
    // direction; a line the operating system lists is `confirmed` (specification 11, "Confidence").
    let mut sink = Recorded::default();
    report(vec![listed_line()], &mut sink);

    assert_eq!(sink.candidates.len(), 1);
    let candidate = &sink.candidates[0];
    assert_eq!(candidate.confidence, "confirmed");
    assert_eq!(candidate.params["chip"], json!("gpiochip0"));
    assert_eq!(candidate.params["label"], json!("pinctrl-bcm2835"));
    assert_eq!(candidate.params["line"], json!(17));
    assert_eq!(candidate.params["name"], json!("GPIO17"));
    assert_eq!(candidate.params["consumer"], json!("led"));
    assert_eq!(candidate.params["used"], json!(true));
    assert_eq!(candidate.params["direction"], json!("out"));
    assert_eq!(candidate.warnings, Vec::<String>::new());
}

#[test]
fn gpio_source_warns_for_a_line_the_sysfs_fallback_listed() {
    // Specification 11, "Sources": the sysfs interface is only the fallback for a kernel without
    // GPIO character devices, and every result from it says so in a warning.
    let mut line = listed_line();
    line.from_sysfs = true;
    let mut sink = Recorded::default();
    report(vec![line], &mut sink);

    assert_eq!(sink.candidates.len(), 1);
    assert_eq!(sink.candidates[0].warnings.len(), 1);
    assert!(
        sink.candidates[0].warnings[0].contains("sysfs"),
        "the sysfs fallback result must say so: {:?}",
        sink.candidates[0].warnings
    );
}
