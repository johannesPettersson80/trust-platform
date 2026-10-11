//! Exact section sizes and conservative, disjoint symbol attribution from LLD maps.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const CODE_CAPACITY: u64 = 448 * 1024;
const SECTOR0_ORIGIN: u64 = 0x0800_0000;
const SECTOR0_CAPACITY: u64 = 16 * 1024;
const FAMILIES: &[&str] = &[
    "validator",
    "preparation",
    "dispatch",
    "engine",
    "decode",
    "encode",
    "btree",
    "sort",
    "stdlib",
    "libm",
    "allocator",
    "firmware",
    "adapter",
    "other",
];

#[derive(Clone, Copy)]
struct Section {
    vma: u64,
    lma: u64,
    bytes: u64,
}
struct Map {
    sections: BTreeMap<String, Section>,
    symbols: BTreeMap<(u64, u64), &'static str>,
    aliases: usize,
}

pub(super) fn compare(current: &str, baseline: &str) -> Result<Value> {
    let current = parse(current).context("current firmware map")?.report()?;
    let baseline = parse(baseline).context("baseline firmware map")?.report()?;
    let delta = |key: &str| -> Result<i64> {
        let current = current[key].as_u64().context("current numeric field")?;
        let baseline = baseline[key].as_u64().context("baseline numeric field")?;
        Ok(i64::try_from(current)? - i64::try_from(baseline)?)
    };
    let section_delta: BTreeMap<_, _> = [
        ".text",
        ".rodata",
        ".vector_table",
        ".firmware_rodata",
        ".data",
        ".bss",
    ]
    .into_iter()
    .map(|name| {
        let now = current["sections"][name]["bytes"].as_u64().unwrap_or(0);
        let old = baseline["sections"][name]["bytes"].as_u64().unwrap_or(0);
        Ok((name, i64::try_from(now)? - i64::try_from(old)?))
    })
    .collect::<Result<_>>()?;
    let family_delta: BTreeMap<_, _> = FAMILIES
        .iter()
        .map(|name| {
            let now = current["approximate_text_families"][name]["bytes"]
                .as_u64()
                .unwrap_or(0);
            let old = baseline["approximate_text_families"][name]["bytes"]
                .as_u64()
                .unwrap_or(0);
            Ok((*name, i64::try_from(now)? - i64::try_from(old)?))
        })
        .collect::<Result<_>>()?;
    Ok(json!({
        "schema": "trust-f401-map-comparison-v1",
        "code_capacity_bytes": CODE_CAPACITY,
        "flash_load_delta_bytes": delta("flash_load_bytes")?,
        "code_span_delta_bytes": delta("code_span_bytes")?,
        "section_delta_bytes": section_delta,
        "approximate_text_family_delta_bytes": family_delta,
        "current": current, "baseline": baseline,
        "limitations": "Section totals are exact map values; flash load excludes RAM-only and debug sections, and does not include gaps. Upper code and sector-0 spans include their placement gaps separately. Family ownership is approximate, disjoint symbol attribution, not isolated causal savings per edit. Upper overflow is estimated against L2's 448 KiB code capacity for both maps; sector-0 capacity is 16 KiB. Only ELF inspection establishes installability. Unnamed instructions, literal pools and alignment remain an explicit residual."
    }))
}

fn parse(text: &str) -> Result<Map> {
    let mut lines = text.lines();
    let header = lines.next().context("empty linker map")?;
    ensure!(
        header
            .split_whitespace()
            .eq(["VMA", "LMA", "Size", "Align", "Out", "In", "Symbol"]),
        "unsupported LLD map header"
    );
    let out = header.find("Out").context("missing output column")?;
    let symbol = header.find("Symbol").context("missing symbol column")?;
    let mut map = Map {
        sections: BTreeMap::new(),
        symbols: BTreeMap::new(),
        aliases: 0,
    };
    let mut in_text = false;
    for (line_number, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let prefix = line
            .get(..out)
            .with_context(|| format!("short row {}", line_number + 2))?;
        let fields: Vec<_> = prefix.split_whitespace().collect();
        ensure!(
            fields.len() == 4,
            "invalid map fields at row {}",
            line_number + 2
        );
        let number =
            |field: &str| u64::from_str_radix(field, 16).context("invalid map hexadecimal field");
        let vma = number(fields[0])?;
        let lma = number(fields[1])?;
        let bytes = number(fields[2])?;
        ensure!(
            vma <= u64::from(u32::MAX)
                && lma <= u64::from(u32::MAX)
                && bytes <= u64::from(u32::MAX),
            "map is not ARM32"
        );
        fields[3].parse::<u32>().context("invalid map alignment")?;
        let tail = &line[out..];
        let name = tail.trim_start();
        let column = out + tail.len() - name.len();
        if column == out && name.starts_with('.') && !name.contains(char::is_whitespace) {
            ensure!(
                map.sections
                    .insert(name.into(), Section { vma, lma, bytes })
                    .is_none(),
                "duplicate output section {name}"
            );
            in_text = name == ".text";
        } else if in_text && column >= symbol && bytes != 0 && !name.starts_with('$') {
            // Thumb function pointers carry bit zero; byte ranges do not.
            let address = vma & !1;
            let family = family(name);
            match map.symbols.entry((address, bytes)) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(family);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    map.aliases += 1;
                    // Conflicting aliases have no defensible single owner.
                    if *entry.get() != family {
                        entry.insert("other");
                    }
                }
            }
        }
    }
    for name in [".text", ".rodata", ".vector_table", ".data", ".bss"] {
        ensure!(
            map.sections.contains_key(name),
            "missing output section {name}"
        );
    }
    Ok(map)
}

impl Map {
    fn report(&self) -> Result<Value> {
        let text = self.sections.get(".text").context("missing text")?;
        let text_end = text
            .vma
            .checked_add(text.bytes)
            .context("text range overflow")?;
        let mut families: BTreeMap<_, u64> = FAMILIES.iter().map(|name| (*name, 0)).collect();
        let mut cursor = text.vma;
        let mut attributed = 0_u64;
        for ((address, bytes), family) in &self.symbols {
            let end = address
                .checked_add(*bytes)
                .context("symbol range overflow")?;
            ensure!(
                *address >= text.vma && end <= text_end,
                "symbol outside text"
            );
            ensure!(*address >= cursor, "overlapping non-alias symbol ranges");
            cursor = end;
            *families.get_mut(family).context("unknown symbol family")? += bytes;
            attributed += bytes;
        }
        let mut flash_load = 0_u64;
        let mut code_end = text.lma;
        let mut code_load = 0_u64;
        let mut sector0_end = SECTOR0_ORIGIN;
        let mut sector0_load = 0_u64;
        let mut loads = Vec::new();
        let mut sections = BTreeMap::new();
        for (name, section) in &self.sections {
            sections.insert(
                name,
                json!({"vma": section.vma, "lma": section.lma, "bytes": section.bytes}),
            );
            if name.starts_with(".debug")
                || matches!(name.as_str(), ".bss" | ".uninit" | ".noinit")
                || !(0x0800_0000..0x1000_0000).contains(&section.lma)
            {
                continue;
            }
            let end = section
                .lma
                .checked_add(section.bytes)
                .context("load range overflow")?;
            if section.bytes != 0 {
                loads.push((section.lma, end));
            }
            flash_load = flash_load
                .checked_add(section.bytes)
                .context("flash sum overflow")?;
            if matches!(name.as_str(), ".vector_table" | ".firmware_rodata") {
                ensure!(
                    section.lma >= SECTOR0_ORIGIN
                        && section.lma < SECTOR0_ORIGIN + SECTOR0_CAPACITY,
                    "vector/data section does not start in sector 0"
                );
                sector0_end = sector0_end.max(end);
                sector0_load = sector0_load
                    .checked_add(section.bytes)
                    .context("sector-0 load sum overflow")?;
            } else {
                ensure!(section.lma >= text.lma, "load precedes executable region");
                code_end = code_end.max(end);
                code_load = code_load
                    .checked_add(section.bytes)
                    .context("upper load sum overflow")?;
            }
        }
        loads.sort_unstable();
        ensure!(
            loads.windows(2).all(|pair| pair[0].1 <= pair[1].0),
            "overlapping output load sections"
        );
        let code_span = code_end - text.lma;
        let sector0_span = sector0_end - SECTOR0_ORIGIN;
        let families: BTreeMap<_, _> = families
            .into_iter()
            .map(|(name, bytes)| (name, json!({"bytes": bytes})))
            .collect();
        Ok(json!({
            "sections": sections,
            "flash_load_bytes": flash_load,
            "code_origin": text.lma,
            "code_span_bytes": code_span,
            "estimated_L2_overflow_bytes": code_span.saturating_sub(CODE_CAPACITY),
            "code_free_bytes": CODE_CAPACITY.saturating_sub(code_span),
            "code_load_bytes": code_load,
            "code_padding_bytes": code_span - code_load,
            "sector0_span_bytes": sector0_span,
            "sector0_load_bytes": sector0_load,
            "sector0_padding_bytes": sector0_span - sector0_load,
            "sector0_free_bytes": SECTOR0_CAPACITY.saturating_sub(sector0_span),
            "sector0_overflow_bytes": sector0_span.saturating_sub(SECTOR0_CAPACITY),
            "approximate_text_families": families,
            "text_attributed_bytes": attributed,
            "text_unmapped_residual_bytes": text.bytes - attributed,
            "deduplicated_alias_rows": self.aliases,
        }))
    }
}

fn family(name: &str) -> &'static str {
    // Generic collection symbols may mention a core type argument. Classify the
    // implementation owner before matching any enclosing runtime type names.
    for (needle, family) in [
        ("alloc::collections::btree", "btree"),
        ("core::slice::sort", "sort"),
        ("trust_runtime_core::sort::", "sort"),
        ("trust_runtime_core::bytecode::validate::", "validator"),
        ("trust_runtime_core::vm::prepared::", "preparation"),
        ("trust_runtime_core::vm::dispatch", "dispatch"),
        ("trust_runtime_core::vm::call::", "dispatch"),
        ("trust_runtime_core::vm::engine::", "engine"),
        ("trust_runtime_core::bytecode::decode::", "decode"),
        ("trust_runtime_core::bytecode::encode::", "encode"),
        ("trust_runtime_core::stdlib::", "stdlib"),
        ("libm::", "libm"),
        ("compiler_builtins::float::", "libm"),
        ("linked_list_allocator::", "allocator"),
        ("embedded_alloc::", "allocator"),
        ("__rustc::__rust_", "allocator"),
        ("trust_nucleo_f401re::", "firmware"),
        ("trust_platform_stm32f4::", "adapter"),
    ] {
        if name.contains(needle) {
            return family;
        }
    }
    "other"
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEADER: &str = "     VMA      LMA     Size Align Out     In      Symbol\n";
    fn row(vma: u32, lma: u32, bytes: u32, indent: usize, name: &str) -> String {
        format!(
            "{vma:8x} {lma:8x} {bytes:8x}     1 {}{name}\n",
            " ".repeat(indent)
        )
    }
    fn fixture(text_bytes: u32) -> String {
        let mut text = HEADER.to_owned();
        text += &row(0x0800_0000, 0x0800_0000, 8, 0, ".vector_table");
        text += &row(0x0801_0000, 0x0801_0000, text_bytes, 0, ".text");
        text += &row(
            0x0801_0001,
            0x0801_0001,
            4,
            16,
            "trust_runtime_core::vm::engine::run",
        );
        text += &row(
            0x0801_0001,
            0x0801_0001,
            4,
            16,
            "trust_runtime_core::vm::engine::alias",
        );
        text += &row(
            0x0801_0005,
            0x0801_0005,
            4,
            16,
            "alloc::collections::btree::Map<trust_runtime_core::vm::engine::State>::insert",
        );
        text += &row(
            0x0801_0000 + text_bytes,
            0x0801_0000 + text_bytes,
            8,
            0,
            ".rodata",
        );
        text += &row(0x2000_0000, 0x0801_0008 + text_bytes, 4, 0, ".data");
        text += &row(0x2000_0004, 0x2000_0004, 100, 0, ".bss");
        text += &row(0, 0, 1000, 0, ".debug_info");
        text
    }
    #[test]
    fn totals_exclude_debug_and_ram_and_aliases_are_counted_once() {
        let report = compare(&fixture(16), &fixture(24)).unwrap();
        assert_eq!(report["current"]["flash_load_bytes"], 36);
        assert_eq!(report["flash_load_delta_bytes"], -8);
        assert_eq!(report["section_delta_bytes"][".text"], -8);
        assert_eq!(report["current"]["deduplicated_alias_rows"], 1);
        assert_eq!(
            report["current"]["approximate_text_families"]["engine"]["bytes"],
            4
        );
        assert_eq!(
            report["current"]["approximate_text_families"]["btree"]["bytes"],
            4
        );
        assert_eq!(report["current"]["text_unmapped_residual_bytes"], 8);
        assert_eq!(report["current"]["code_span_bytes"], 28);
    }
    #[test]
    fn failed_link_map_still_reports_capacity_overflow() {
        let report = compare(&fixture(448 * 1024), &fixture(16)).unwrap();
        assert_eq!(report["current"]["estimated_L2_overflow_bytes"], 12);
    }
    #[test]
    fn zero_size_flash_alignment_reserves_span_without_load_bytes() {
        let baseline = fixture(448 * 1024);
        let aligned = baseline.clone() + &row(0x0808_0020, 0x0808_0020, 0, 0, ".gnu.sgstubs");
        let report = compare(&aligned, &baseline).unwrap();
        assert_eq!(report["flash_load_delta_bytes"], 0);
        assert_eq!(report["code_span_delta_bytes"], 20);
        assert_eq!(report["current"]["estimated_L2_overflow_bytes"], 32);
    }

    #[test]
    fn sector0_constants_are_reported_separately_from_upper_load_and_padding() {
        let old = fixture(16);
        let new = old.clone() + &row(0x0800_0010, 0x0800_0010, 12, 0, ".firmware_rodata");
        let report = compare(&new, &old).unwrap();
        assert_eq!(report["flash_load_delta_bytes"], 12);
        assert_eq!(report["code_span_delta_bytes"], 0);
        assert_eq!(report["current"]["code_span_bytes"], 28);
        assert_eq!(report["current"]["code_load_bytes"], 28); // Includes RAM .data's load image.
        assert_eq!(report["current"]["sector0_load_bytes"], 20);
        assert_eq!(report["current"]["sector0_span_bytes"], 28);
        assert_eq!(report["current"]["sector0_padding_bytes"], 8);
        assert_eq!(report["current"]["sector0_free_bytes"], 16 * 1024 - 28);
        let overflow =
            old.clone() + &row(0x0800_0010, 0x0800_0010, 16 * 1024, 0, ".firmware_rodata");
        assert_eq!(
            compare(&overflow, &old).unwrap()["current"]["sector0_overflow_bytes"],
            16
        );
        let overlap = old.clone() + &row(0x0800_0004, 0x0800_0004, 12, 0, ".firmware_rodata");
        assert!(compare(&overlap, &old).is_err());
    }

    #[test]
    fn incomplete_malformed_and_overlapping_maps_are_rejected() {
        assert!(compare("", &fixture(16)).is_err());
        assert!(compare(&fixture(16).replace(".rodata", "missing"), &fixture(16)).is_err());
        assert!(compare(&fixture(16).replace("8000000", "not_hex"), &fixture(16)).is_err());
        let overlapping = fixture(16).replace(" 8010005", " 8010003");
        assert!(compare(&overlapping, &fixture(16)).is_err());
    }
}
