use std::fmt::Write as _;
use std::path::Path;

pub(super) struct Series<'a> {
    pub name: &'a str,
    pub color: &'a str,
    pub dashed: bool,
    pub x: &'a [f64],
    pub y: &'a [f64],
}

pub(super) struct Panel<'a> {
    pub title: &'a str,
    pub x_unit: &'a str,
    pub y_unit: &'a str,
    pub x_minimum: f64,
    pub x_maximum: f64,
    pub y_minimum: f64,
    pub y_maximum: f64,
    pub series: Vec<Series<'a>>,
}

pub(super) struct RetainedSeries<'a> {
    pub panel: &'a str,
    pub name: &'a str,
    pub x: &'a [f64],
    pub y: &'a [f64],
    pub x_unit: &'a str,
    pub y_unit: &'a str,
}

pub(super) fn retain(
    root: &Path,
    stem: &str,
    title: &str,
    subtitle: &str,
    panels: &[Panel<'_>],
    retained: &[RetainedSeries<'_>],
    summary: &str,
) {
    assert_eq!(panels.len(), 2, "classical lesson figures use two panels");
    let mut trace = String::from("panel,series,point,x,y,x_unit,y_unit\n");
    for panel in panels {
        for series in &panel.series {
            assert_eq!(series.x.len(), series.y.len(), "x/y evidence length");
            for (point, (x, y)) in series.x.iter().zip(series.y).enumerate() {
                writeln!(
                    trace,
                    "{},{},{},{:.12},{:.12},{},{}",
                    csv(panel.title),
                    csv(series.name),
                    point + 1,
                    x,
                    y,
                    csv(panel.x_unit),
                    csv(panel.y_unit)
                )
                .unwrap();
            }
        }
    }
    for series in retained {
        assert_eq!(
            series.x.len(),
            series.y.len(),
            "retained x/y evidence length"
        );
        for (point, (x, y)) in series.x.iter().zip(series.y).enumerate() {
            writeln!(
                trace,
                "{},{},{},{:.12},{:.12},{},{}",
                csv(series.panel),
                csv(series.name),
                point + 1,
                x,
                y,
                csv(series.x_unit),
                csv(series.y_unit)
            )
            .unwrap();
        }
    }
    write_or_compare_curriculum_artifact(&root.join(format!("{stem}-trace.csv")), &trace);
    write_or_compare_curriculum_artifact(&root.join(format!("{stem}-summary.csv")), summary);
    write_or_compare_curriculum_artifact(
        &root.join(format!("{stem}.svg")),
        &svg(title, subtitle, panels),
    );
}

fn write_or_compare_curriculum_artifact(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("curriculum evidence parent"))
            .expect("create curriculum evidence directory");
        std::fs::write(path, expected).expect("write curriculum evidence");
    } else {
        let actual = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read retained artifact {}: {error}", path.display()));
        assert_eq!(
            actual,
            expected,
            "retained artifact drift: {}",
            path.display()
        );
    }
}

fn svg(title: &str, subtitle: &str, panels: &[Panel<'_>]) -> String {
    let title_lines = wrapped_lines(title, 58);
    let subtitle_lines = wrapped_lines(subtitle, 92);
    let last_title_y = 42.0 + title_lines.len().saturating_sub(1) as f64 * 28.0;
    let subtitle_top = last_title_y + 28.0;
    let last_subtitle_y = subtitle_top + subtitle_lines.len().saturating_sub(1) as f64 * 18.0;
    let legend_y = (last_subtitle_y + 30.0).max(108.0);
    let first_panel_top = legend_y + 49.0;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" role=\"img\" aria-labelledby=\"title desc\" viewBox=\"0 0 900 650\">\n<title id=\"title\">{}</title>\n<desc id=\"desc\">{}</desc>\n<defs><marker id=\"arrow\" markerWidth=\"8\" markerHeight=\"8\" refX=\"7\" refY=\"3\" orient=\"auto\"><path d=\"M0,0 L0,6 L8,3 z\" fill=\"#243044\"/></marker></defs>\n<rect width=\"900\" height=\"650\" fill=\"#f7f9fc\"/>\n<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:#243044}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:#d8e0eb;stroke-width:1}}</style>\n",
        xml(title), xml(subtitle)
    );
    for (index, line) in title_lines.iter().enumerate() {
        writeln!(
            out,
            "<text x=\"55\" y=\"{}\" font-size=\"24\" font-weight=\"700\">{}</text>",
            42.0 + index as f64 * 28.0,
            xml(line)
        )
        .unwrap();
    }
    for (index, line) in subtitle_lines.iter().enumerate() {
        writeln!(
            out,
            "<text x=\"55\" y=\"{}\" font-size=\"14\">{}</text>",
            subtitle_top + index as f64 * 18.0,
            xml(line)
        )
        .unwrap();
    }
    let mut legend_x = 55.0;
    for series in &panels[0].series {
        let width = 45.0 + series.name.len() as f64 * 7.0;
        if legend_x + width > 845.0 {
            break;
        }
        let dash = if series.dashed {
            " stroke-dasharray=\"7 5\""
        } else {
            ""
        };
        writeln!(out, "<line x1=\"{legend_x:.0}\" y1=\"{legend_y:.0}\" x2=\"{:.0}\" y2=\"{legend_y:.0}\" stroke=\"{}\" stroke-width=\"3\"{dash}/><text x=\"{:.0}\" y=\"{}\" font-size=\"12\">{}</text>", legend_x + 28.0, series.color, legend_x + 34.0, legend_y + 5.0, xml(series.name)).unwrap();
        legend_x += width;
    }
    for (index, panel) in panels.iter().enumerate() {
        let top = first_panel_top + index as f64 * 250.0;
        let height = if panel.title == "Discrete poles" {
            180.0
        } else {
            160.0
        };
        let full_width = 780.0;
        let width = if panel.title == "Discrete poles" {
            (height * (panel.x_maximum - panel.x_minimum) / (panel.y_maximum - panel.y_minimum))
                .min(full_width)
        } else {
            full_width
        };
        let left = 75.0 + (full_width - width) / 2.0;
        writeln!(
            out,
            "<text x=\"{left}\" y=\"{}\" font-size=\"16\" font-weight=\"700\">{} ({})</text>",
            top - 12.0,
            xml(panel.title),
            xml(panel.y_unit)
        )
        .unwrap();
        let y_tick_count = if panel.y_unit == "code" { 6 } else { 4 };
        for tick in 0..=y_tick_count {
            let fraction = tick as f64 / y_tick_count as f64;
            let y = top + height * fraction;
            let value = panel.y_maximum - (panel.y_maximum - panel.y_minimum) * fraction;
            writeln!(out, "<line class=\"grid\" x1=\"{left}\" y1=\"{y:.2}\" x2=\"{}\" y2=\"{y:.2}\"/><text x=\"{}\" y=\"{:.2}\" text-anchor=\"end\" font-size=\"11\">{value:.2}</text>", left + width, left - 8.0, y + 4.0).unwrap();
        }
        for tick in 0..=4 {
            let fraction = tick as f64 / 4.0;
            let x = left + width * fraction;
            let value = panel.x_minimum + (panel.x_maximum - panel.x_minimum) * fraction;
            writeln!(out, "<line class=\"grid\" x1=\"{x:.2}\" y1=\"{top}\" x2=\"{x:.2}\" y2=\"{}\"/><text x=\"{x:.2}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\">{value:.2}</text>", top + height, top + height + 18.0).unwrap();
        }
        for series in &panel.series {
            let mut points = String::new();
            for (x_value, y_value) in series.x.iter().zip(series.y) {
                let x = left
                    + width
                        * ((*x_value - panel.x_minimum) / (panel.x_maximum - panel.x_minimum))
                            .clamp(0.0, 1.0);
                let y = top
                    + height
                        * (1.0
                            - ((*y_value - panel.y_minimum) / (panel.y_maximum - panel.y_minimum))
                                .clamp(0.0, 1.0));
                write!(points, "{x:.2},{y:.2} ").unwrap();
            }
            if series.x.len() <= 3 {
                for point in points.split_whitespace() {
                    let (x, y) = point.split_once(',').expect("rendered evidence point");
                    writeln!(
                        out,
                        "<circle cx=\"{x}\" cy=\"{y}\" r=\"4\" fill=\"{}\"/>",
                        series.color
                    )
                    .unwrap();
                }
            } else {
                let dash = if series.dashed {
                    " stroke-dasharray=\"7 5\""
                } else {
                    ""
                };
                let arrow = if panel.title.contains("Nyquist") {
                    " marker-end=\"url(#arrow)\""
                } else {
                    ""
                };
                writeln!(out, "<polyline fill=\"none\" stroke=\"{}\" stroke-width=\"2.2\"{dash}{arrow} points=\"{}\"/>", series.color, points.trim_end()).unwrap();
            }
        }
        writeln!(out, "<rect class=\"axis\" x=\"{left}\" y=\"{top}\" width=\"{width}\" height=\"{height}\" fill=\"none\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\">{}</text>", left + width / 2.0, top + height + 36.0, xml(panel.x_unit)).unwrap();
    }
    out.push_str("</svg>\n");
    out
}

fn wrapped_lines(value: &str, maximum_characters: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        if !current.is_empty() && current.len() + word.len() + 1 > maximum_characters {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn csv(value: &str) -> String {
    value.replace(',', ";")
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
