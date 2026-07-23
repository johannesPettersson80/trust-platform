use super::parse_ads_toml;

fn connection(name: &str, target_net_id: &str, local_net_id: Option<&str>, point: &str) -> String {
    let local_net_id = local_net_id
        .map(|value| format!("local_net_id = {value:?}\n"))
        .unwrap_or_default();
    format!(
        r#"
[[connections]]
name = {name:?}
target_net_id = {target_net_id:?}
host = "192.0.2.10"
ams_port = 851
{local_net_id}transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Value"
var = {point:?}
type = "DINT"
"#
    )
}

fn connection_with_point_fields(point_fields: &str) -> String {
    format!(
        r#"
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.0.2.10"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
var = "line1_value"
{point_fields}
"#
    )
}

fn assert_invalid(label: &str, text: &str, expected: &str) {
    let error = match parse_ads_toml(text) {
        Err(error) => error,
        Ok(_) => panic!("invalid ADS configuration case {label:?} must fail"),
    };
    assert!(
        error.to_string().contains(expected),
        "case {label:?}: expected error containing {expected:?}, got {error}"
    );
}

#[test]
fn rejects_duplicate_connection_names_with_distinct_point_bindings() {
    let text = format!(
        "{}{}",
        connection("line1", "5.23.91.12.1.1", None, "line1_value"),
        connection("line1", "5.23.91.13.1.1", None, "line2_value"),
    );

    let error = parse_ads_toml(&text).expect_err("duplicate connection names must fail");

    assert!(error.to_string().contains("connection name 'line1'"));
    assert!(error.to_string().contains("unique"));
}

#[test]
fn rejects_noncanonical_target_ams_net_ids_during_configuration_parse() {
    for target_net_id in [
        "1.2.3.4.5",
        "1.2.3.4.5.x",
        "256.2.3.4.5.6",
        "01.2.3.4.5.6",
        " 1.2.3.4.5.6",
    ] {
        let text = connection("line1", target_net_id, None, "line1_value");

        let error = match parse_ads_toml(&text) {
            Err(error) => error,
            Ok(_) => panic!("invalid target AMS Net ID {target_net_id:?} must fail"),
        };

        assert!(error.to_string().contains("invalid target AMS Net ID"));
        assert!(error
            .to_string()
            .contains("six decimal octets in canonical form"));
    }
}

#[test]
fn rejects_blank_or_noncanonical_explicit_local_ams_net_ids_during_configuration_parse() {
    for local_net_id in [
        "",
        "1.2.3.4.5",
        "1.2.3.4.5.x",
        "256.2.3.4.5.6",
        "01.2.3.4.5.6",
        " 1.2.3.4.5.6",
    ] {
        let text = connection("line1", "5.23.91.12.1.1", Some(local_net_id), "line1_value");

        let error = match parse_ads_toml(&text) {
            Err(error) => error,
            Ok(_) => panic!("invalid explicit local AMS Net ID {local_net_id:?} must fail"),
        };

        assert!(error.to_string().contains("invalid local AMS Net ID"));
        assert!(error
            .to_string()
            .contains("six decimal octets in canonical form"));
    }
}

#[test]
fn locks_connection_and_declared_global_shape_validation() {
    let valid = connection("line1", "5.23.91.12.1.1", None, "line1_value");
    let no_points = valid
        .split("[[connections.points]]")
        .next()
        .expect("connection header");
    let cases = [
        (
            "empty connection list",
            "connections = []".to_string(),
            "requires at least one [[connections]] entry",
        ),
        (
            "blank connection name",
            valid.replace("name = \"line1\"", "name = \"   \""),
            "connections.name must not be empty",
        ),
        (
            "blank host",
            valid.replace("host = \"192.0.2.10\"", "host = \"   \""),
            "connections.host must not be empty",
        ),
        (
            "zero AMS port",
            valid.replace("ams_port = 851", "ams_port = 0"),
            "ams_port must be >= 1",
        ),
        (
            "connection without points",
            format!("{no_points}points = []\n"),
            "requires at least one point",
        ),
        (
            "blank declared global",
            valid.replace("var = \"line1_value\"", "var = \"   \""),
            "connections.points.var must not be empty",
        ),
        (
            "unknown transport",
            valid.replace("transport = \"plain\"", "transport = \"unknown\""),
            "invalid ADS transport",
        ),
    ];

    for (label, text, expected) in cases {
        assert_invalid(label, &text, expected);
    }
}

#[test]
fn locks_exact_point_address_shape_validation() {
    let cases = [
        (
            "blank symbol",
            "symbol = \"   \"\ntype = \"DINT\"",
            "requires symbol or index_group/index_offset/size",
        ),
        (
            "missing address",
            "type = \"DINT\"",
            "requires symbol or index_group/index_offset/size",
        ),
        (
            "missing index group",
            "index_offset = 0\nsize = 4\ntype = \"DINT\"",
            "index_group is required",
        ),
        (
            "missing index offset",
            "index_group = 16416\nsize = 4\ntype = \"DINT\"",
            "index_offset is required",
        ),
        (
            "missing index size",
            "index_group = 16416\nindex_offset = 0\ntype = \"DINT\"",
            "size is required",
        ),
        (
            "zero index size",
            "index_group = 16416\nindex_offset = 0\nsize = 0\ntype = \"DINT\"",
            "size must be >= 1",
        ),
    ];

    for (label, point_fields, expected) in cases {
        assert_invalid(label, &connection_with_point_fields(point_fields), expected);
    }
}

#[test]
fn locks_point_enum_and_type_validation() {
    let cases = [
        (
            "unknown access",
            "symbol = \"MAIN.Value\"\ntype = \"DINT\"\naccess = \"unknown\"",
            "invalid ADS point access",
        ),
        (
            "unknown update mode",
            "symbol = \"MAIN.Value\"\ntype = \"DINT\"\nmode = \"unknown\"",
            "invalid ADS point mode",
        ),
        (
            "unknown notification mode",
            "symbol = \"MAIN.Value\"\ntype = \"DINT\"\nmode = \"notify\"\nnotification_mode = \"unknown\"",
            "invalid ADS notification_mode",
        ),
        (
            "unsupported IEC type",
            "symbol = \"MAIN.Value\"\ntype = \"STRUCT\"",
            "unsupported ADS IEC type",
        ),
        (
            "missing STRING capacity",
            "symbol = \"MAIN.Value\"\ntype = \"STRING\"",
            "ADS STRING type requires a length",
        ),
        (
            "malformed inline STRING capacity",
            "symbol = \"MAIN.Value\"\ntype = \"STRING(nope)\"",
            "invalid ADS STRING length",
        ),
        (
            "overflowing inline STRING capacity",
            "symbol = \"MAIN.Value\"\ntype = \"STRING(65536)\"",
            "invalid ADS STRING length",
        ),
        (
            "descending array dimension",
            "symbol = \"MAIN.Value\"\ntype = \"DINT\"\ndimensions = [{ lower = 2, upper = 1 }]",
            "array dimension upper 1 is below lower 2",
        ),
    ];

    for (label, point_fields, expected) in cases {
        assert_invalid(label, &connection_with_point_fields(point_fields), expected);
    }
}

#[test]
fn rejects_zero_string_capacity() {
    for point_fields in [
        "symbol = \"MAIN.Value\"\ntype = \"STRING(0)\"",
        "symbol = \"MAIN.Value\"\ntype = \"STRING\"\nstring_len = 0",
    ] {
        assert_invalid(
            "zero STRING capacity",
            &connection_with_point_fields(point_fields),
            "ADS STRING capacity must be at least 1",
        );
    }
}

#[test]
fn rejects_string_capacity_on_non_string_type() {
    assert_invalid(
        "string_len on non-STRING",
        &connection_with_point_fields("symbol = \"MAIN.Value\"\ntype = \"DINT\"\nstring_len = 8"),
        "string_len is valid only for STRING",
    );
}

#[test]
fn rejects_index_address_size_that_disagrees_with_declared_type() {
    for (label, point_fields, expected) in [
        (
            "scalar DINT size mismatch",
            "index_group = 16416\nindex_offset = 0\nsize = 2\ntype = \"DINT\"",
            "address size 2 does not match declared type size 4",
        ),
        (
            "DINT array size mismatch",
            "index_group = 16416\nindex_offset = 0\nsize = 4\ntype = \"DINT\"\ndimensions = [{ lower = 1, upper = 2 }]",
            "address size 4 does not match declared type size 8",
        ),
    ] {
        assert_invalid(
            label,
            &connection_with_point_fields(point_fields),
            expected,
        );
    }
}

#[test]
fn rejects_array_metadata_whose_byte_length_overflows() {
    assert_invalid(
        "array byte length overflow",
        &connection_with_point_fields(
            "symbol = \"MAIN.Value\"\ntype = \"LREAL\"\ndimensions = [{ lower = -9223372036854775808, upper = 9223372036854775807 }]",
        ),
        "ADS type metadata byte length overflows",
    );
}
