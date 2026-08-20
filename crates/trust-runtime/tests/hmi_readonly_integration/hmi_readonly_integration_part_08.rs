use super::*;

#[test]
fn hmi_responsive_layout_breakpoint_classes_cover_mobile_tablet_desktop() {
    let js_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/web/ui/hmi.js");
    let script = r#"
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');

class ClassList {
  constructor() { this.values = new Set(); }
  add(...names) { for (const name of names) { if (name) { this.values.add(String(name)); } } }
  remove(...names) { for (const name of names) { this.values.delete(String(name)); } }
  contains(name) { return this.values.has(String(name)); }
  entries() { return Array.from(this.values.values()); }
}

const noop = () => {};
const sourcePath = process.env.HMI_JS_PATH;
const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { state, applyResponsiveLayout, viewportForWidth };';
const bodyClassList = new ClassList();
const context = {
  console,
  URLSearchParams,
  window: {
    location: { protocol: 'http:', host: '127.0.0.1:7777', search: '' },
    addEventListener: noop,
    setInterval: () => 1,
    clearInterval: noop,
    setTimeout: () => 1,
    clearTimeout: noop,
    innerWidth: 1280,
  },
  document: {
    getElementById: () => null,
    body: { classList: bodyClassList },
    documentElement: { style: { setProperty: noop } },
  },
  fetch: async () => { throw new Error('unexpected fetch'); },
  DOMParser: class {
    parseFromString() {
      return { querySelector: () => null, documentElement: null };
    }
  },
};
vm.createContext(context);
vm.runInContext(source, context, { filename: 'hmi.js' });

const test = context.__hmi_test__;
test.state.schema = {
  responsive: {
    mode: 'auto',
    mobile_max_px: 680,
    tablet_max_px: 1024,
  },
};

assert.strictEqual(test.viewportForWidth(500, 680, 1024), 'mobile');
assert.strictEqual(test.viewportForWidth(900, 680, 1024), 'tablet');
assert.strictEqual(test.viewportForWidth(1400, 680, 1024), 'desktop');

context.window.innerWidth = 520;
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-mobile'), true);
assert.strictEqual(bodyClassList.contains('viewport-tablet'), false);
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), false);

context.window.innerWidth = 900;
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-mobile'), false);
assert.strictEqual(bodyClassList.contains('viewport-tablet'), true);
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), false);

context.window.innerWidth = 1440;
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-mobile'), false);
assert.strictEqual(bodyClassList.contains('viewport-tablet'), false);
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), false);

test.state.schema.responsive.mode = 'kiosk';
test.applyResponsiveLayout();
assert.strictEqual(bodyClassList.contains('viewport-kiosk'), true);
console.log('ok');
"#;
    run_node_hmi_script(&js_path, script, "responsive breakpoint classes");
}

#[test]
fn hmi_process_asset_pack_templates_and_bindings_align() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hmi_root = repo_root.join("hmi");
    assert!(hmi_root.is_dir(), "hmi/ directory is missing");

    let symbols_root = hmi_root.join("pid-symbols");
    assert!(
        symbols_root.is_dir(),
        "hmi/pid-symbols/ directory is missing"
    );
    assert!(
        symbols_root
            .join("LICENSE-EQUINOR-ENGINEERING-SYMBOLS.txt")
            .is_file(),
        "symbol library license file is missing"
    );

    let symbol_svg_count = fs::read_dir(&symbols_root)
        .expect("read hmi/pid-symbols directory")
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        })
        .count();
    assert!(
        symbol_svg_count >= 20,
        "expected symbol library to contain many SVGs, found {symbol_svg_count}"
    );

    let plant_svg = repository_source_tree_read_to_string!(
        (hmi_root.join("plant.svg"), &repo_root),
        roots = ["hmi"],
        extension = "svg",
    )
    .expect("read hmi/plant.svg");
    let minimal_svg = repository_source_tree_read_to_string!(
        (hmi_root.join("plant-minimal.svg"), &repo_root),
        roots = ["hmi"],
        extension = "svg",
    )
    .expect("read hmi/plant-minimal.svg");
    let bindings_example = repository_source_tree_read_to_string!(
        (hmi_root.join("plant.bindings.example.toml"), &repo_root),
        roots = ["hmi"],
        extension = "toml",
    )
    .expect("read hmi/plant.bindings.example.toml");

    assert!(
        bindings_example.contains("svg = \"plant.svg\""),
        "bindings example must target hmi/plant.svg"
    );

    let plant_ids = extract_svg_ids(&plant_svg);
    let minimal_ids = extract_svg_ids(&minimal_svg);
    let required_ids = [
        "pid-tank-001-level-fill",
        "pid-tank-002-level-fill",
        "pid-pump-001-status",
        "pid-valve-001-status",
        "pid-line-002",
        "pid-tag-fit-001-pv",
        "pid-tag-pt-001-pv",
        "pid-tag-tank-001-level",
        "pid-tag-tank-002-level",
        "pid-tag-pump-001-state",
        "pid-tag-valve-001-position",
        "pid-banner-alarm-001",
        "pid-banner-alarm-001-text",
    ];
    for required_id in required_ids {
        assert!(
            plant_ids.contains(required_id),
            "plant.svg is missing stable id '{required_id}'"
        );
        assert!(
            minimal_ids.contains(required_id),
            "plant-minimal.svg is missing stable id '{required_id}'"
        );
    }

    let selectors = extract_quoted_values_from_lines(&bindings_example, "selector = ");
    assert!(
        !selectors.is_empty(),
        "bindings example selectors must not be empty"
    );
    for selector in selectors {
        if let Some(id) = selector.strip_prefix('#') {
            assert!(
                plant_ids.contains(id),
                "binding selector '{selector}' is missing in plant.svg"
            );
            assert!(
                minimal_ids.contains(id),
                "binding selector '{selector}' is missing in plant-minimal.svg"
            );
        }
    }

    let sources = extract_quoted_values_from_lines(&bindings_example, "source = ");
    assert!(
        !sources.is_empty(),
        "bindings example must include bind source paths"
    );
    for source in sources {
        assert!(
            source.contains('.'),
            "source '{source}' should use canonical Program.field or global.name form"
        );
    }
}

#[test]
fn hmi_polling_stays_under_cycle_budget() {
    let state = hmi_control_state(hmi_fixture_source());
    let base = start_test_server(state);
    let schema = post_control(&base, "hmi.schema.get", None);
    let widgets = schema
        .get("result")
        .and_then(|v| v.get("widgets"))
        .and_then(|v| v.as_array())
        .expect("schema widgets");
    let ids = widgets
        .iter()
        .filter_map(|widget| widget.get("id").and_then(|v| v.as_str()))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty(), "ids must not be empty");

    let cycle_budget = Duration::from_millis(100);
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    let polls: u32 = 240;

    for _ in 0..polls {
        let started = Instant::now();
        let values = post_control(&base, "hmi.values.get", Some(json!({ "ids": ids.clone() })));
        let elapsed = started.elapsed();
        total += elapsed;
        max = max.max(elapsed);
        assert_eq!(values.get("ok").and_then(|v| v.as_bool()), Some(true));
    }

    let avg = total / polls;
    assert!(
        max < cycle_budget,
        "max hmi.values.get latency {:?} exceeded cycle budget {:?}",
        max,
        cycle_budget
    );
    assert!(
        avg < Duration::from_millis(30),
        "average hmi.values.get latency {:?} exceeded expected polling overhead",
        avg
    );
}
