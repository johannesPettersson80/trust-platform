use std::fmt::Write as _;
use std::path::Path;

pub(super) struct Series<'a> {
    pub name: &'a str,
    pub color: &'a str,
    pub dashed: bool,
    pub values: &'a [f64],
}

pub(super) struct Panel<'a> {
    pub title: &'a str,
    pub unit: &'a str,
    pub minimum: f64,
    pub maximum: f64,
    pub series: Vec<Series<'a>>,
}

pub(super) struct Marker<'a> {
    pub at_seconds: f64,
    pub label: &'a str,
}

pub(super) fn retain(
    root: &Path,
    stem: &str,
    title: &str,
    subtitle: &str,
    sample_seconds: f64,
    panels: &[Panel<'_>],
    retained_only: &[Series<'_>],
    markers: &[Marker<'_>],
    summary: &str,
) {
    retain_axis(
        root,
        stem,
        title,
        subtitle,
        "sample",
        "time_s",
        sample_seconds,
        "time (s)",
        panels,
        retained_only,
        markers,
        summary,
    );
}

#[allow(clippy::too_many_arguments, dead_code)]
pub(super) fn retain_with_trace(
    root: &Path,
    stem: &str,
    title: &str,
    subtitle: &str,
    sample_seconds: f64,
    panels: &[Panel<'_>],
    markers: &[Marker<'_>],
    trace: &str,
    summary: &str,
) {
    let samples = panels
        .iter()
        .flat_map(|panel| panel.series.iter())
        .next()
        .expect("curriculum evidence series")
        .values
        .len();
    assert!(panels
        .iter()
        .flat_map(|panel| panel.series.iter())
        .all(|item| item.values.len() == samples));
    super::write_or_compare_curriculum_artifact(&root.join(format!("{stem}-trace.csv")), trace);
    super::write_or_compare_curriculum_artifact(&root.join(format!("{stem}-summary.csv")), summary);
    super::write_or_compare_curriculum_artifact(
        &root.join(format!("{stem}.svg")),
        &svg(
            title,
            subtitle,
            sample_seconds * samples as f64,
            "time (s)",
            panels,
            markers,
        ),
    );
}

#[allow(dead_code)]
pub(super) fn retain_sequence(
    root: &Path,
    stem: &str,
    title: &str,
    subtitle: &str,
    panels: &[Panel<'_>],
    retained_only: &[Series<'_>],
    markers: &[Marker<'_>],
    summary: &str,
) {
    retain_axis(
        root,
        stem,
        title,
        subtitle,
        "case",
        "case_index",
        1.0,
        "decision case",
        panels,
        retained_only,
        markers,
        summary,
    );
}

pub(super) fn retain_updates(
    root: &Path,
    stem: &str,
    title: &str,
    subtitle: &str,
    panels: &[Panel<'_>],
    retained_only: &[Series<'_>],
    markers: &[Marker<'_>],
    summary: &str,
) {
    retain_axis(
        root,
        stem,
        title,
        subtitle,
        "update",
        "update_index",
        1.0,
        "controller update",
        panels,
        retained_only,
        markers,
        summary,
    );
}

#[allow(clippy::too_many_arguments)]
fn retain_axis(
    root: &Path,
    stem: &str,
    title: &str,
    subtitle: &str,
    row_name: &str,
    x_column: &str,
    x_step: f64,
    x_axis_label: &str,
    panels: &[Panel<'_>],
    retained_only: &[Series<'_>],
    markers: &[Marker<'_>],
    summary: &str,
) {
    let mut series = panels
        .iter()
        .flat_map(|panel| panel.series.iter())
        .collect::<Vec<_>>();
    series.extend(retained_only.iter());
    let samples = series.first().expect("motion evidence series").values.len();
    assert!(series.iter().all(|item| item.values.len() == samples));

    let mut csv = format!("{row_name},{x_column}");
    for item in &series {
        write!(csv, ",{}", csv_name(item.name)).unwrap();
    }
    csv.push('\n');
    for index in 0..samples {
        write!(csv, "{},{:.6}", index + 1, (index + 1) as f64 * x_step).unwrap();
        for item in &series {
            write!(csv, ",{:.12}", item.values[index]).unwrap();
        }
        csv.push('\n');
    }

    super::write_or_compare_curriculum_artifact(&root.join(format!("{stem}-trace.csv")), &csv);
    super::write_or_compare_curriculum_artifact(&root.join(format!("{stem}-summary.csv")), summary);
    super::write_or_compare_curriculum_artifact(
        &root.join(format!("{stem}.svg")),
        &svg(
            title,
            subtitle,
            x_step * samples as f64,
            x_axis_label,
            panels,
            markers,
        ),
    );
}

pub(super) fn retain_frequency(
    root: &Path,
    stem: &str,
    title: &str,
    subtitle: &str,
    frequency_step_hz: f64,
    panels: &[Panel<'_>],
    summary: &str,
) {
    let series = panels
        .iter()
        .flat_map(|panel| panel.series.iter())
        .collect::<Vec<_>>();
    let samples = series
        .first()
        .expect("frequency evidence series")
        .values
        .len();
    assert!(series.iter().all(|item| item.values.len() == samples));
    let mut csv = String::from("bin,frequency_hz");
    for item in &series {
        write!(csv, ",{}", csv_name(item.name)).unwrap();
    }
    csv.push('\n');
    for index in 0..samples {
        write!(
            csv,
            "{},{:.6}",
            index + 1,
            (index + 1) as f64 * frequency_step_hz
        )
        .unwrap();
        for item in &series {
            write!(csv, ",{:.12}", item.values[index]).unwrap();
        }
        csv.push('\n');
    }
    super::write_or_compare_curriculum_artifact(&root.join(format!("{stem}-trace.csv")), &csv);
    super::write_or_compare_curriculum_artifact(&root.join(format!("{stem}-summary.csv")), summary);
    super::write_or_compare_curriculum_artifact(
        &root.join(format!("{stem}.svg")),
        &svg(
            title,
            subtitle,
            frequency_step_hz * samples as f64,
            "frequency (Hz)",
            panels,
            &[],
        ),
    );
}

fn svg(
    title: &str,
    subtitle: &str,
    duration: f64,
    x_axis_label: &str,
    panels: &[Panel<'_>],
    markers: &[Marker<'_>],
) -> String {
    assert_eq!(
        panels.len(),
        2,
        "motion teaching figures use two aligned panels"
    );
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" role=\"img\" aria-labelledby=\"title desc\" viewBox=\"0 0 900 610\">\n<title id=\"title\">{}</title>\n<desc id=\"desc\">{}</desc>\n<rect width=\"900\" height=\"610\" fill=\"#f7f9fc\"/>\n<style>text{{font-family:system-ui,-apple-system,sans-serif;fill:#243044}} .axis{{stroke:#526078;stroke-width:1}} .grid{{stroke:#d8e0eb;stroke-width:1}} .marker{{stroke:#b43c32;stroke-width:1.5;stroke-dasharray:4 4}}</style>\n<text x=\"55\" y=\"42\" font-size=\"24\" font-weight=\"700\">{}</text>\n<text x=\"55\" y=\"68\" font-size=\"14\">{}</text>\n",
        xml(title), xml(subtitle), xml(title), xml(subtitle)
    );

    let mut legend_x = 55.0;
    let mut legend_y = 96.0;
    let all_legend_series = panels
        .iter()
        .flat_map(|panel| panel.series.iter())
        .collect::<Vec<_>>();
    let legend_series = if all_legend_series.len() > 12 {
        panels[0].series.iter().collect::<Vec<_>>()
    } else {
        all_legend_series
    };
    for item in legend_series {
        let item_width = 42.0 + item.name.len() as f64 * 7.2;
        if legend_x + item_width > 845.0 {
            legend_x = 55.0;
            legend_y += 20.0;
        }
        let dash = if item.dashed {
            " stroke-dasharray=\"7 5\""
        } else {
            ""
        };
        write!(out, "<line x1=\"{legend_x:.0}\" y1=\"{legend_y:.0}\" x2=\"{:.0}\" y2=\"{legend_y:.0}\" stroke=\"{}\" stroke-width=\"3\"{dash}/><text x=\"{:.0}\" y=\"{:.0}\" font-size=\"12\">{}</text>\n", legend_x + 28.0, item.color, legend_x + 34.0, legend_y + 5.0, xml(item.name)).unwrap();
        legend_x += item_width;
    }

    let first_panel_top = 155.0_f64.max(legend_y + 35.0);
    let panel_height = if first_panel_top > 155.0 {
        145.0
    } else {
        160.0
    };
    for (panel_index, panel) in panels.iter().enumerate() {
        let top = if panel_index == 0 {
            first_panel_top
        } else {
            first_panel_top + 205.0
        };
        let left = 70.0;
        let width = 790.0;
        let height = panel_height;
        write!(
            out,
            "<text x=\"{left}\" y=\"{:.0}\" font-size=\"16\" font-weight=\"700\">{} ({})</text>\n",
            top - 12.0,
            xml(panel.title),
            xml(panel.unit)
        )
        .unwrap();
        for tick in 0..=4 {
            let y = top + height * tick as f64 / 4.0;
            let value = panel.maximum - (panel.maximum - panel.minimum) * tick as f64 / 4.0;
            let tick_label = if (panel.maximum - panel.minimum).abs() < 0.01 {
                format!("{value:.4}")
            } else {
                format!("{value:.2}")
            };
            write!(out, "<line class=\"grid\" x1=\"{left}\" y1=\"{y:.2}\" x2=\"{}\" y2=\"{y:.2}\"/><text x=\"{}\" y=\"{:.2}\" text-anchor=\"end\" font-size=\"11\">{tick_label}</text>\n", left + width, left - 8.0, y + 4.0).unwrap();
        }
        let sequence_axis = matches!(x_axis_label, "decision case" | "controller update");
        let tick_divisions = if sequence_axis {
            panel
                .series
                .first()
                .expect("panel series")
                .values
                .len()
                .saturating_sub(1)
                .max(1)
        } else {
            6
        };
        for tick in 0..=tick_divisions {
            let x = left + width * tick as f64 / tick_divisions as f64;
            let tick_label = if sequence_axis {
                (tick + 1).to_string()
            } else {
                format!("{:.1}", duration * tick as f64 / tick_divisions as f64)
            };
            write!(out, "<line class=\"grid\" x1=\"{x:.2}\" y1=\"{top}\" x2=\"{x:.2}\" y2=\"{}\"/><text x=\"{x:.2}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\">{tick_label}</text>\n", top + height, top + height + 18.0).unwrap();
        }
        for marker in markers {
            let x = left + width * marker.at_seconds / duration;
            write!(
                out,
                "<line class=\"marker\" x1=\"{x:.2}\" y1=\"{top}\" x2=\"{x:.2}\" y2=\"{}\"/>\n",
                top + height
            )
            .unwrap();
            if panel_index == 0 {
                let (label_x, anchor) = if x > left + width - 90.0 {
                    (x - 5.0, "end")
                } else {
                    (x + 5.0, "start")
                };
                write!(
                    out,
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\" font-size=\"11\" fill=\"#8c2f28\">{}</text>\n",
                    label_x,
                    top + 14.0,
                    anchor,
                    xml(marker.label)
                )
                .unwrap();
            }
        }
        for item in &panel.series {
            let mut points = String::new();
            for (index, value) in item.values.iter().enumerate() {
                let x = left + width * index as f64 / (item.values.len() - 1).max(1) as f64;
                let normalized =
                    ((*value - panel.minimum) / (panel.maximum - panel.minimum)).clamp(0.0, 1.0);
                let y = top + height * (1.0 - normalized);
                write!(points, "{x:.2},{y:.2} ").unwrap();
            }
            let dash = if item.dashed {
                " stroke-dasharray=\"7 5\""
            } else {
                ""
            };
            write!(
                out,
                "<polyline fill=\"none\" stroke=\"{}\" stroke-width=\"2.2\"{} points=\"{}\"/>\n",
                item.color,
                dash,
                points.trim_end()
            )
            .unwrap();
        }
        write!(out, "<rect class=\"axis\" x=\"{left}\" y=\"{top}\" width=\"{width}\" height=\"{height}\" fill=\"none\"/>\n").unwrap();
    }
    write!(
        out,
        "<text x=\"465\" y=\"598\" text-anchor=\"middle\" font-size=\"12\">{}</text>\n</svg>\n",
        xml(x_axis_label)
    )
    .unwrap();
    out
}

fn csv_name(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace([' ', '/', '(', ')'], "_")
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
