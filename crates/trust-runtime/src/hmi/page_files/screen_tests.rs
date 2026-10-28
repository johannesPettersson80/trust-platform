//! Specification 39, §4.1 and §15: the screen layout file.

use super::*;

const SCREEN_FILE: &str = "hmi/screen.toml";

/// The screen example of specification 39, §15.3.
const SCREEN_EXAMPLE: &str = r##"format = 1
size = [1920, 1080]
theme = "light"
symbols = "flat"

[top_bar]
title = "Feed station"
clock = true
alarms = true
user = true

[navigation]
style = "side"

[colors.light]
accent = "#0e7490"
"##;

fn screen_errors(text: &str) -> Vec<PageFileError> {
    parse_screen(SCREEN_FILE, text).expect_err("the screen file should be rejected")
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

#[test]
fn spec_example_screen_loads() {
    let screen = parse_screen(SCREEN_FILE, SCREEN_EXAMPLE).expect("the example loads");
    assert_eq!(
        screen,
        HmiScreen {
            start_page: None,
            size: [1920, 1080],
            theme: HmiTheme::Light,
            symbols: HmiSymbolStyle::Flat,
            languages: vec!["en".to_string()],
            top_bar: Some(HmiTopBar {
                title: "Feed station".to_string(),
                logo: None,
                clock: true,
                alarms: true,
                user: true,
                page_title: false,
            }),
            navigation: HmiNavigation {
                style: HmiNavigationStyle::Side,
                position: HmiNavigationPosition::Left,
            },
            colors: HmiColors {
                dark: Default::default(),
                light: [("accent".to_string(), "#0e7490".to_string())].into(),
            },
        }
    );
}

#[test]
fn spec_example_screen_is_written_back_byte_for_byte() {
    let screen = parse_screen(SCREEN_FILE, SCREEN_EXAMPLE).expect("the example loads");
    assert_eq!(write_screen(&screen), SCREEN_EXAMPLE);
}

#[test]
fn every_screen_field_round_trips() {
    let screen = HmiScreen {
        start_page: None,
        size: [1280, 800],
        theme: HmiTheme::Dark,
        symbols: HmiSymbolStyle::Flat,
        languages: vec!["de".to_string(), "en".to_string()],
        top_bar: Some(HmiTopBar {
            title: "Line 2".to_string(),
            logo: Some("hmi/assets/logo.svg".to_string()),
            clock: false,
            alarms: true,
            user: false,
            page_title: true,
        }),
        navigation: HmiNavigation {
            style: HmiNavigationStyle::Side,
            position: HmiNavigationPosition::Right,
        },
        colors: HmiColors {
            dark: [
                ("accent".to_string(), "#ff8800".to_string()),
                ("page".to_string(), "#000".to_string()),
            ]
            .into(),
            light: [("alarm".to_string(), "#c01c28cc".to_string())].into(),
        },
    };
    let written = write_screen(&screen);
    assert_eq!(parse_screen(SCREEN_FILE, &written).expect(&written), screen);
}

#[test]
fn screen_without_top_bar_or_navigation_uses_the_defaults() {
    let text = "format = 1\nsize = [1024, 768]\ntheme = \"dark\"\nsymbols = \"flat\"\n";
    let screen = parse_screen(SCREEN_FILE, text).expect("the screen loads");
    assert_eq!(screen.top_bar, None);
    assert_eq!(
        screen.navigation,
        HmiNavigation {
            style: HmiNavigationStyle::Side,
            position: HmiNavigationPosition::Left,
        }
    );
    assert_eq!(
        write_screen(&screen),
        format!("{text}\n[navigation]\nstyle = \"side\"\n")
    );
}

#[test]
fn unknown_screen_key_is_reported_with_its_line() {
    let text = SCREEN_EXAMPLE.replace("clock = true", "clock = true\ncolour = \"red\"");
    let errors = screen_errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&text, "colour"));
    assert!(
        errors[0].message.contains("colour"),
        "{}",
        errors[0].message
    );
}

#[test]
fn unknown_theme_is_reported_with_its_line() {
    let text = SCREEN_EXAMPLE.replace("theme = \"light\"", "theme = \"blue\"");
    let errors = screen_errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&text, "theme = \"blue\""));
    assert!(errors[0].message.contains("light"), "{}", errors[0].message);
}

/// §14.5: a screen lists its languages, the first being the one its texts are written in;
/// without the key, English.
#[test]
fn a_screen_lists_its_languages_the_first_being_that_of_its_texts() {
    let text = SCREEN_EXAMPLE.replace(
        "symbols = \"flat\"\n",
        "symbols = \"flat\"\nlanguages = [\"sv\", \"en\", \"zh-Hans\"]\n",
    );
    let screen = parse_screen(SCREEN_FILE, &text).expect(&text);
    assert_eq!(screen.languages, ["sv", "en", "zh-Hans"]);
    assert_eq!(write_screen(&screen), text);
    let plain = parse_screen(SCREEN_FILE, SCREEN_EXAMPLE).expect("the example loads");
    assert_eq!(plain.languages, ["en"]);
}

#[test]
fn a_language_that_is_no_language_tag_is_reported_with_its_line() {
    let text = SCREEN_EXAMPLE.replace(
        "symbols = \"flat\"\n",
        "symbols = \"flat\"\nlanguages = [\"en\", \"Swedish\"]\n",
    );
    let errors = screen_errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&text, "languages = "));
    assert!(
        errors[0].message.contains("Swedish"),
        "{}",
        errors[0].message
    );
}

/// §4.1 and §14.2: the symbols are the two-dimensional `flat` ones; the rendered style was
/// dropped with decision 7 of 2 October 2026.
#[test]
fn the_rendered_symbol_style_is_reported_with_its_line() {
    let text = SCREEN_EXAMPLE.replace("symbols = \"flat\"", "symbols = \"rendered\"");
    assert!(
        parse_screen(SCREEN_FILE, &text).is_err(),
        "a screen with the rendered symbol style loads"
    );
    let errors = screen_errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&text, "symbols = \"rendered\""));
    assert!(errors[0].message.contains("flat"), "{}", errors[0].message);
}

#[test]
fn zero_size_is_reported_with_its_line() {
    let text = SCREEN_EXAMPLE.replace("size = [1920, 1080]", "size = [0, 1080]");
    assert_eq!(
        screen_errors(&text),
        vec![PageFileError {
            file: SCREEN_FILE.to_string(),
            line: line_of(&text, "size = [0, 1080]"),
            message: "size must be at least 1 × 1 pixels".to_string(),
        }]
    );
}

#[test]
fn side_position_without_a_side_bar_is_reported() {
    let text = SCREEN_EXAMPLE.replace("style = \"side\"", "style = \"tabs\"\nposition = \"right\"");
    let errors = screen_errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].message.contains("side bar"),
        "{}",
        errors[0].message
    );
}

#[test]
fn unsupported_screen_format_is_reported() {
    let text = SCREEN_EXAMPLE.replacen("format = 1", "format = 3", 1);
    assert_eq!(
        screen_errors(&text),
        vec![PageFileError {
            file: SCREEN_FILE.to_string(),
            line: 1,
            message: "format 3 is not supported; this truST reads format 1".to_string(),
        }]
    );
}

/// Specification 39, §14.1: project colours name known tokens.
#[test]
fn an_unknown_colour_token_is_reported_with_its_line() {
    let text = format!("{SCREEN_EXAMPLE}\n[colors.dark]\nacent = \"#ff8800\"\n");
    let errors = screen_errors(&text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].line, line_of(&text, "acent ="));
    assert!(
        errors[0].message.contains("`acent` is not a colour token"),
        "{}",
        errors[0].message
    );
    assert!(
        errors[0].message.contains("accent-2"),
        "the message lists the tokens: {}",
        errors[0].message
    );
}

/// Specification 39, §14.1: project colours are hex colours, nothing else.
#[test]
fn a_colour_that_is_not_hex_is_reported_with_its_line() {
    for value in [
        "red",
        "#12345",
        "rgb(1, 2, 3)",
        "#22d3ee; background: url(x)",
    ] {
        let text = format!("{SCREEN_EXAMPLE}\n[colors.dark]\naccent = \"{value}\"\n");
        let errors = screen_errors(&text);
        assert_eq!(errors.len(), 1, "{value}: {errors:?}");
        assert_eq!(
            errors[0].line,
            line_of(&text, &format!("accent = \"{value}\"")),
            "{value}"
        );
        assert!(
            errors[0].message.contains("`accent` must be a hex colour"),
            "{value}: {}",
            errors[0].message
        );
    }
}

/// Specification 39, §14.1: the trend pen colours are tokens a project may set.
#[test]
fn the_pen_colours_can_be_set() {
    let text = "format = 1\nsize = [1280, 800]\ntheme = \"dark\"\nsymbols = \"flat\"\n\n[colors.dark]\npen-1 = \"#ff8800\"\npen-4 = \"#00ff00\"\n";
    let screen = parse_screen(SCREEN_FILE, text).expect("the screen loads");
    assert_eq!(
        screen.colors.dark.get("pen-1").map(String::as_str),
        Some("#ff8800")
    );
    assert_eq!(
        screen.colors.dark.get("pen-4").map(String::as_str),
        Some("#00ff00")
    );
}

#[test]
fn a_selected_start_page_round_trips_and_malformed_names_are_rejected() {
    let text = "format=1\nsize=[1280,720]\ntheme='dark'\nsymbols='flat'\nstart_page='overview'\n";
    let screen = parse_screen(SCREEN_FILE, text).unwrap();
    assert_eq!(screen.start_page.as_deref(), Some("overview"));
    assert_eq!(
        parse_screen(SCREEN_FILE, &super::write_screen(&screen)).unwrap(),
        screen
    );
    assert!(parse_screen(SCREEN_FILE, &text.replace("overview", "../other")).is_err());
}
