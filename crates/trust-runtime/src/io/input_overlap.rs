//! Composed drivers that fill the same bytes of the input image (specification 11 §6.5). Each
//! driver reads into the one image in the configured order, so a later driver overwrites what an
//! earlier one read into the bytes both fill. A composition may mean that, so the runtime does not
//! refuse it; it warns at every program start.

use std::ops::Range;

use smol_str::SmolStr;

/// Two composed drivers that fill the same bytes of the input image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputOverlap {
    /// The driver earlier in the configured order, by its link (`<protocol>:<position>`,
    /// specification 42 §2) or, without one, by its name.
    pub earlier: SmolStr,
    /// The later driver, which overwrites what the earlier one read into `bytes`.
    pub later: SmolStr,
    /// The bytes both fill, within the image.
    pub bytes: Range<usize>,
}

/// The overlaps of `drivers`, each named with the input bytes it declares (specification 42 §5
/// item 4), in the configured order, within an input image of `image_len` bytes: for each two
/// drivers, each joined range of the bytes both fill, in the order of the drivers and then of the
/// bytes. A driver that declares no bytes overlaps nothing.
pub fn input_overlaps(
    drivers: &[(SmolStr, Vec<Range<usize>>)],
    image_len: usize,
) -> Vec<InputOverlap> {
    let mut overlaps = Vec::new();
    for (position, (earlier, earlier_bytes)) in drivers.iter().enumerate() {
        for (later, later_bytes) in &drivers[position + 1..] {
            let shared = earlier_bytes
                .iter()
                .flat_map(|first| {
                    later_bytes.iter().map(move |second| {
                        first.start.max(second.start)..first.end.min(second.end).min(image_len)
                    })
                })
                .collect();
            // Two ranges that share no byte give an empty or reversed range, which joining drops.
            overlaps.extend(
                super::joined_bytes(shared)
                    .into_iter()
                    .map(|bytes| InputOverlap {
                        earlier: earlier.clone(),
                        later: later.clone(),
                        bytes,
                    }),
            );
        }
    }
    overlaps
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bytes of a Modbus/TCP block or a raw MQTT input topic: the image from its start.
    const WHOLE_IMAGE: (usize, usize) = (0, usize::MAX);

    /// A driver by its link `name` that declares the bytes `start..end` of each pair of `bytes`.
    fn driver(name: &str, bytes: &[(usize, usize)]) -> (SmolStr, Vec<Range<usize>>) {
        let bytes = bytes.iter().map(|&(start, end)| start..end).collect();
        (SmolStr::new(name), bytes)
    }

    fn overlap(earlier: &str, later: &str, bytes: Range<usize>) -> InputOverlap {
        InputOverlap {
            earlier: SmolStr::new(earlier),
            later: SmolStr::new(later),
            bytes,
        }
    }

    #[test]
    fn a_point_beside_a_raw_topic_overlaps_in_the_points_byte() {
        // Specification 11 §6.5 and specification 42 §5 item 4: a Modbus/TCP input point at byte
        // 0 fills 0..1, and a raw MQTT input topic fills the whole image; in a 2-byte image they
        // share byte 0 alone, the point's driver first. Catches an overlap that is missed, drivers
        // named in the wrong order, and the topic's whole range reported.
        let drivers = [
            driver("modbus_tcp:0", &[(0, 1)]),
            driver("mqtt:1", &[WHOLE_IMAGE]),
        ];
        assert_eq!(
            input_overlaps(&drivers, 2),
            vec![overlap("modbus_tcp:0", "mqtt:1", 0..1)]
        );
    }

    #[test]
    fn two_drivers_that_fill_the_whole_image_overlap_in_every_byte_of_it() {
        // A Modbus/TCP block and a raw MQTT topic both fill the image from its start: in a 2-byte
        // image they share bytes 0..2, not 0..usize::MAX. Catches a range not cut to the image.
        let drivers = [
            driver("modbus_tcp:0", &[WHOLE_IMAGE]),
            driver("mqtt:1", &[WHOLE_IMAGE]),
        ];
        assert_eq!(
            input_overlaps(&drivers, 2),
            vec![overlap("modbus_tcp:0", "mqtt:1", 0..2)]
        );
    }

    #[test]
    fn points_and_mappings_in_separate_bytes_do_not_overlap() {
        // The multi_driver example: the Modbus/TCP point fills byte 0 and the MQTT mapping of
        // %IX1.0 byte 1. Ranges that touch share no byte. Catches touching ranges counted as an
        // overlap.
        let drivers = [
            driver("modbus_tcp:0", &[(0, 1)]),
            driver("mqtt:1", &[(1, 2)]),
        ];
        assert_eq!(input_overlaps(&drivers, 2), Vec::new());
    }

    #[test]
    fn each_two_drivers_are_compared_in_their_order() {
        // Three drivers in an 8-byte image: the first fills 0..4, the second 2..6, the third
        // 3..5. The first and the second share 2..4, the first and the third 3..4, the second and
        // the third 3..5, reported in that order. Catches only neighbours compared, and a pair
        // reported twice or the later driver first.
        let drivers = [
            driver("modbus_tcp:0", &[(0, 4)]),
            driver("ethercat:1", &[(2, 6)]),
            driver("mqtt:2", &[(3, 5)]),
        ];
        assert_eq!(
            input_overlaps(&drivers, 8),
            vec![
                overlap("modbus_tcp:0", "ethercat:1", 2..4),
                overlap("modbus_tcp:0", "mqtt:2", 3..4),
                overlap("ethercat:1", "mqtt:2", 3..5),
            ]
        );
    }

    #[test]
    fn a_pair_that_shares_two_ranges_reports_each() {
        // The first fills 0..2 and 4..6, the second 1..5: they share 1..2 and 4..5, which do not
        // touch, so each is reported. Catches the shared bytes merged into one range that claims
        // 2..4.
        let drivers = [
            driver("modbus_tcp:0", &[(0, 2), (4, 6)]),
            driver("mqtt:1", &[(1, 5)]),
        ];
        assert_eq!(
            input_overlaps(&drivers, 8),
            vec![
                overlap("modbus_tcp:0", "mqtt:1", 1..2),
                overlap("modbus_tcp:0", "mqtt:1", 4..5),
            ]
        );
    }

    #[test]
    fn nothing_overlaps_outside_the_image_or_with_a_driver_that_declares_nothing() {
        // A point at byte 4 beside a raw topic, in a 2-byte image: they share no byte of the
        // image. A driver that declares no bytes, such as `simulated`, overlaps nothing, and an
        // empty image holds no shared byte. Catches bytes past the image reported, and a driver
        // without declared bytes taken to fill the whole image.
        let past_the_image = [
            driver("modbus_tcp:0", &[(4, 5)]),
            driver("mqtt:1", &[WHOLE_IMAGE]),
        ];
        assert_eq!(input_overlaps(&past_the_image, 2), Vec::new());
        let declares_nothing = [driver("simulated:0", &[]), driver("mqtt:1", &[WHOLE_IMAGE])];
        assert_eq!(input_overlaps(&declares_nothing, 2), Vec::new());
        let two_blocks = [
            driver("modbus_tcp:0", &[WHOLE_IMAGE]),
            driver("mqtt:1", &[WHOLE_IMAGE]),
        ];
        assert_eq!(input_overlaps(&two_blocks, 0), Vec::new());
    }
}
