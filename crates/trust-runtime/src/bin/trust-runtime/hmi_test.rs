//! `trust-runtime hmi test` (specification 39, §18 phase 5 item 6): every page drawn in headless
//! Chromium with the editor's design samples, as the runtime's drawing module draws it, and
//! compared with its approved screenshot `hmi/screens/<page>.png`. A page whose look changed, or
//! has no approved screenshot, fails the command and leaves its new look as
//! `hmi/screens/<page>.actual.png`; `--update` approves every page's new look.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context};
use serde_json::json;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::my_symbols::symbols_in;
use trust_runtime::hmi::page_files::{parse_page, parse_screen, HmiNavigationStyle};

/// A pixel differs when a channel differs by more than this; a page changed when more than this
/// share of its pixels differ.
const CHANNEL: u8 = 24;
const SHARE: f64 = 0.002;

pub fn run_hmi_test(
    project: Option<PathBuf>,
    update: bool,
    chromium: Option<PathBuf>,
) -> anyhow::Result<()> {
    let root = match project {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let bundle = HmiBundle::read(&root)
        .with_context(|| format!("read the HMI files of {}", root.display()))?;
    let screen_text = bundle
        .files
        .get("hmi/screen.toml")
        .ok_or_else(|| anyhow!("the project has no screen layout (hmi/screen.toml)"))?;
    let screen = parse_screen("hmi/screen.toml", &String::from_utf8_lossy(screen_text)).map_err(
        |errors| {
            anyhow!(
                "hmi/screen.toml does not load: {}",
                errors
                    .first()
                    .map(|error| error.message.clone())
                    .unwrap_or_default()
            )
        },
    )?;
    // A page is drawn in the page area: the design size without the top bar and the navigation.
    let side = if screen.navigation.style == HmiNavigationStyle::Side {
        232
    } else {
        0
    };
    let above = if screen.top_bar.is_some() { 56 } else { 0 }
        + if screen.navigation.style == HmiNavigationStyle::Tabs {
            48
        } else {
            0
        };
    let size = [
        screen.size[0].saturating_sub(side).max(1),
        screen.size[1].saturating_sub(above).max(1),
    ];
    let chromium = find_chromium(chromium)?;
    let (symbols, _) = symbols_in(&bundle.files);
    let work = tempfile_dir()?;
    let (drawing, font) = trust_runtime::web::drawing_assets();
    std::fs::write(work.join("drawing.js"), drawing)?;
    std::fs::write(work.join("inter.woff2"), font)?;
    let screens = root.join("hmi/screens");
    let mut failed = 0;
    for (path, bytes) in bundle
        .files
        .iter()
        .filter(|(path, _)| path.starts_with("hmi/pages/") && path.ends_with(".toml"))
    {
        let name = path
            .trim_start_matches("hmi/pages/")
            .trim_end_matches(".toml")
            .to_string();
        let page = parse_page(path, &String::from_utf8_lossy(bytes)).map_err(|errors| {
            anyhow!(
                "{path} does not load: {}",
                errors
                    .first()
                    .map(|error| error.message.clone())
                    .unwrap_or_default()
            )
        })?;
        let placed = trust_runtime::web::placed_faceplates(&root, &name);
        let data = json!({ "page": page, "screen": screen, "size": size, "symbols": symbols, "catalogue": Catalogue::shipped(), "placed": placed });
        let html = work.join(format!("{name}.html"));
        std::fs::write(&html, page_html(&serde_json::to_string(&data)?))?;
        let shot = work.join(format!("{name}.png"));
        screenshot(&chromium, &html, &shot, size)?;
        let approved = screens.join(format!("{name}.png"));
        let actual = screens.join(format!("{name}.actual.png"));
        if update {
            std::fs::create_dir_all(&screens)?;
            std::fs::copy(&shot, &approved)?;
            let _ = std::fs::remove_file(&actual);
            println!("{name}: approved");
            continue;
        }
        if !approved.is_file() {
            std::fs::create_dir_all(&screens)?;
            std::fs::copy(&shot, &actual)?;
            println!("{name}: no approved screenshot (hmi/screens/{name}.png); its look is in hmi/screens/{name}.actual.png, run with --update to approve it");
            failed += 1;
            continue;
        }
        match compare(&pixels(&approved)?, &pixels(&shot)?) {
            Compared::Same => {
                let _ = std::fs::remove_file(&actual);
                println!("{name}: same");
            }
            Compared::Changed(share) => {
                std::fs::copy(&shot, &actual)?;
                println!("{name}: changed ({:.1} % of the pixels), the new look in hmi/screens/{name}.actual.png", share * 100.0);
                failed += 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// The page laid out for its page area as the runtime display lays it out, drawn with the design
/// samples, filling the window.
fn page_html(data: &str) -> String {
    let data = data.replace("</", "<\\/");
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><style>html, body {{ margin: 0; overflow: hidden; }} #hmi {{ width: 100vw; height: 100vh; }} #hmi > svg {{ display: block; width: 100%; height: 100%; }}</style>
<script src="drawing.js"></script></head>
<body><div id="hmi"></div>
<script>
const data = {data};
const drawing = TrustHmiDrawing;
drawing.registerSymbols(data.symbols);
const style = document.createElement("style");
style.textContent = drawing.tokensCss(data.screen.colors || {{}}, "inter.woff2");
document.head.append(style);
const area = document.getElementById("hmi");
area.className = "hmi-theme-" + (data.screen.theme || "light");
const adapted = drawing.adaptPage(data.page, data.screen.size, data.size);
const values = drawing.designValues(adapted.page, data.catalogue);
for (const entry of Object.values(data.placed)) Object.assign(values, drawing.designValues(entry.model, data.catalogue));
area.innerHTML = drawing.renderToString(drawing.drawPage(adapted.page, {{ size: adapted.size, values, placed: data.placed }}));
</script></body></html>"#
    )
}

/// Draws the page in headless Chromium at the page area's size.
fn screenshot(chromium: &Path, html: &Path, shot: &Path, size: [u32; 2]) -> anyhow::Result<()> {
    let output = Command::new(chromium)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--hide-scrollbars",
            "--force-device-scale-factor=1",
            "--virtual-time-budget=5000",
        ])
        .arg(format!("--window-size={},{}", size[0], size[1]))
        .arg(format!("--screenshot={}", shot.display()))
        .arg(format!("file://{}", html.display()))
        .output()
        .with_context(|| format!("run {}", chromium.display()))?;
    if !shot.is_file() {
        bail!(
            "Chromium drew nothing: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or_default()
        );
    }
    Ok(())
}

/// The Chromium named, else `CHROMIUM`, the usual names on the path, or Playwright's.
fn find_chromium(named: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(path) = named.or_else(|| std::env::var_os("CHROMIUM").map(PathBuf::from)) {
        return Ok(path);
    }
    for candidate in [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
    ] {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return Ok(PathBuf::from(candidate));
        }
    }
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(entries) = std::fs::read_dir(Path::new(&home).join(".cache/ms-playwright"))
    {
        for entry in entries.filter_map(Result::ok) {
            for tail in ["chrome-linux64/chrome", "chrome-linux/chrome"] {
                let path = entry.path().join(tail);
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }
    bail!(
        "hmi test draws the pages in Chromium: install it, or name it with --chromium or CHROMIUM"
    )
}

fn tempfile_dir() -> anyhow::Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("trust-hmi-test-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// A picture's width, height and RGBA pixels.
struct Pixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn pixels(path: &Path) -> anyhow::Result<Pixels> {
    let mut decoder = png::Decoder::new(
        std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?,
    );
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer)?;
    let bytes = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => bytes.to_vec(),
        png::ColorType::Rgb => bytes
            .chunks(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .chunks(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Grayscale => bytes
            .iter()
            .flat_map(|&grey| [grey, grey, grey, 255])
            .collect(),
        png::ColorType::Indexed => bail!("{} is an indexed picture", path.display()),
    };
    Ok(Pixels {
        width: info.width,
        height: info.height,
        rgba,
    })
}

#[derive(Debug, PartialEq)]
enum Compared {
    Same,
    /// The share of the pixels that differ, 1 for another size.
    Changed(f64),
}

fn compare(approved: &Pixels, actual: &Pixels) -> Compared {
    if (approved.width, approved.height) != (actual.width, actual.height) {
        return Compared::Changed(1.0);
    }
    let differing = approved
        .rgba
        .chunks(4)
        .zip(actual.rgba.chunks(4))
        .filter(|(before, after)| {
            before
                .iter()
                .zip(after.iter())
                .any(|(a, b)| a.abs_diff(*b) > CHANNEL)
        })
        .count();
    let share =
        differing as f64 / (f64::from(approved.width) * f64::from(approved.height)).max(1.0);
    if share > SHARE {
        Compared::Changed(share)
    } else {
        Compared::Same
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(width: u32, height: u32, value: u8) -> Pixels {
        Pixels {
            width,
            height,
            rgba: vec![value; (width * height * 4) as usize],
        }
    }

    #[test]
    fn a_picture_is_the_same_within_the_tolerance_and_changed_beyond_it() {
        let approved = flat(100, 100, 10);
        assert_eq!(
            compare(&approved, &flat(100, 100, 30)),
            Compared::Same,
            "a channel off by 20"
        );
        let mut few = flat(100, 100, 10);
        few.rgba[..4 * 20].fill(200);
        assert_eq!(
            compare(&approved, &few),
            Compared::Same,
            "20 pixels of 10 000"
        );
        let mut many = flat(100, 100, 10);
        many.rgba[..4 * 100].fill(200);
        assert_eq!(compare(&approved, &many), Compared::Changed(0.01));
        assert_eq!(
            compare(&approved, &flat(100, 90, 10)),
            Compared::Changed(1.0),
            "another size"
        );
    }
}
