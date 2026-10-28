use super::*;

#[cfg(target_os = "linux")]
#[test]
fn gpio_lines_report_the_chip_offset_name_consumer_use_and_direction() {
    // Specification 11, "Identity": a GPIO line is listed by its chip, the chip's label, its offset
    // within the chip, its name, its consumer, whether it is in use, and its direction.
    let lines = chip_lines(
        "gpiochip0",
        Some("pinctrl-bcm2835"),
        vec![
            ChipLine {
                offset: 17,
                name: Some("GPIO17".to_string()),
                consumer: Some("led".to_string()),
                used: true,
                direction: Some("out".to_string()),
            },
            ChipLine {
                offset: 18,
                name: None,
                consumer: None,
                used: false,
                direction: Some("in".to_string()),
            },
        ],
    );

    assert_eq!(
        lines,
        vec![
            GpioLine {
                chip: "gpiochip0".to_string(),
                label: Some("pinctrl-bcm2835".to_string()),
                line: 17,
                name: Some("GPIO17".to_string()),
                consumer: Some("led".to_string()),
                used: true,
                direction: Some("out".to_string()),
                from_sysfs: false,
            },
            GpioLine {
                chip: "gpiochip0".to_string(),
                label: Some("pinctrl-bcm2835".to_string()),
                line: 18,
                name: None,
                consumer: None,
                used: false,
                direction: Some("in".to_string()),
                from_sysfs: false,
            },
        ]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn gpio_lines_count_the_offset_within_each_chip() {
    // Specification 11, "Identity": the line offset is the offset within its chip, not a sysfs
    // global number, so the same offset on two chips stays two lines.
    let first = chip_lines(
        "gpiochip0",
        None,
        vec![ChipLine {
            offset: 0,
            name: None,
            consumer: None,
            used: false,
            direction: None,
        }],
    );
    let second = chip_lines(
        "gpiochip1",
        None,
        vec![ChipLine {
            offset: 0,
            name: None,
            consumer: None,
            used: false,
            direction: None,
        }],
    );

    assert_eq!(first[0].chip, "gpiochip0");
    assert_eq!(first[0].line, 0);
    assert_eq!(second[0].chip, "gpiochip1");
    assert_eq!(second[0].line, 0);
}

#[test]
fn gpio_lines_from_sysfs_say_so_and_name_no_line_consumer_or_use() {
    // Specification 11, "Sources": a line the sysfs fallback lists is marked, so the search warns,
    // and the sysfs interface gives no line name, consumer or use.
    let line = GpioLine::from_sysfs(GpioLineInfo {
        chip: "gpiochip7".to_string(),
        label: Some("test-chip".to_string()),
        line: 100,
        direction: Some("out".to_string()),
    });

    assert_eq!(
        line,
        GpioLine {
            chip: "gpiochip7".to_string(),
            label: Some("test-chip".to_string()),
            line: 100,
            name: None,
            consumer: None,
            used: false,
            direction: Some("out".to_string()),
            from_sysfs: true,
        }
    );
}
