use super::*;

#[test]
fn hmi_widget_renderers_handle_null_stale_and_good_values() {
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
  toggle(name, force) {
    const value = String(name);
    if (force === true) { this.values.add(value); return true; }
    if (force === false) { this.values.delete(value); return false; }
    if (this.values.has(value)) { this.values.delete(value); return false; }
    this.values.add(value);
    return true;
  }
}

class FakeElement {
  constructor(tag) {
    this.tagName = String(tag || 'div').toUpperCase();
    this.children = [];
    this.attrs = {};
    this.style = {};
    this.dataset = {};
    this.className = '';
    this.classList = new ClassList();
    this.listeners = new Map();
    this.textContent = '';
    this.type = '';
    this.min = '';
    this.max = '';
    this.step = '';
    this.value = '';
    this.disabled = false;
  }
  appendChild(child) {
    this.children.push(child);
    return child;
  }
  setAttribute(key, value) {
    this.attrs[String(key)] = String(value);
  }
  getAttribute(key) {
    return this.attrs[String(key)];
  }
  addEventListener(event, handler) {
    const key = String(event);
    if (!this.listeners.has(key)) {
      this.listeners.set(key, []);
    }
    this.listeners.get(key).push(handler);
  }
  dispatch(event, payload = {}) {
    const handlers = this.listeners.get(String(event)) || [];
    for (const handler of handlers) {
      handler({ target: this, ...payload });
    }
  }
  querySelector(selector) {
    const match = (node) => {
      if (!(node instanceof FakeElement)) {
        return false;
      }
      if (selector.startsWith('.')) {
        const className = selector.slice(1);
        const classes = `${node.className || ''} ${node.attrs.class || ''}`
          .split(/\s+/)
          .filter(Boolean);
        return node.classList.contains(className) || classes.includes(className);
      }
      if (selector.startsWith('#')) {
        return node.attrs.id === selector.slice(1);
      }
      return node.tagName.toLowerCase() === selector.toLowerCase();
    };
    const stack = [...this.children];
    while (stack.length > 0) {
      const node = stack.shift();
      if (match(node)) {
        return node;
      }
      if (node && Array.isArray(node.children)) {
        stack.unshift(...node.children);
      }
    }
    return null;
  }
}

const noop = () => {};
const sourcePath = process.env.HMI_JS_PATH;
const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { state, createWidgetRenderer, applyValueDelta };';
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
    createElement: (tag) => new FakeElement(tag),
    createElementNS: (_ns, tag) => new FakeElement(tag),
    getElementById: () => null,
    body: { classList: new ClassList() },
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
test.state.schema = { read_only: true };

function firstByClass(host, className) {
  const node = host.querySelector(`.${className}`);
  assert(node, `expected .${className}`);
  return node;
}

const gaugeHost = new FakeElement('div');
const gaugeApply = test.createWidgetRenderer(
  { id: 'gauge', widget: 'gauge', label: 'Pump Speed', data_type: 'REAL', writable: false, min: 0, max: 100, unit: 'rpm', zones: [] },
  gaugeHost,
);
gaugeApply(null);
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-label').textContent, 'Pump Speed');
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-center-value').textContent, '--');
gaugeApply({ v: 64, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-center-value').textContent, '64');
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-unit').textContent, 'rpm');
gaugeApply({ v: 12, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-center-value').textContent, '12');
assert.strictEqual(firstByClass(gaugeHost, 'widget-gauge-unit').textContent, 'rpm');
assert.notStrictEqual(firstByClass(gaugeHost, 'widget-gauge-value').attrs.d, '');

const sparkHost = new FakeElement('div');
const sparkApply = test.createWidgetRenderer(
  { id: 'spark', widget: 'sparkline', data_type: 'REAL', writable: false, unit: 'bar' },
  sparkHost,
);
sparkApply(null);
assert.strictEqual(firstByClass(sparkHost, 'widget-sparkline-label').textContent, '--');
sparkApply({ v: 10, q: 'good', ts_ms: 1 });
sparkApply({ v: 15, q: 'stale', ts_ms: 2 });
sparkApply({ v: 20, q: 'good', ts_ms: 3 });
assert.strictEqual(firstByClass(sparkHost, 'widget-sparkline-label').textContent, '20 bar');
assert.ok((firstByClass(sparkHost, 'widget-sparkline-line').attrs.points || '').length > 0);
assert.ok(sparkHost.querySelector('.widget-sparkline-area'), 'sparkline area should exist');

const barHost = new FakeElement('div');
const barApply = test.createWidgetRenderer(
  { id: 'bar', widget: 'bar', data_type: 'REAL', writable: false, min: 0, max: 100, unit: '%' },
  barHost,
);
barApply(null);
assert.strictEqual(firstByClass(barHost, 'widget-bar-label').textContent, '--');
barApply({ v: 45, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(barHost, 'widget-bar-label').textContent, '45 %');
barApply({ v: 80, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(barHost, 'widget-bar-label').textContent, '80 %');
assert.notStrictEqual(firstByClass(barHost, 'widget-bar-fill').style.width, '0%');

const tankHost = new FakeElement('div');
const tankApply = test.createWidgetRenderer(
  { id: 'tank', widget: 'tank', data_type: 'REAL', writable: false, min: 0, max: 100, unit: '%' },
  tankHost,
);
tankApply(null);
assert.strictEqual(firstByClass(tankHost, 'widget-tank-label').textContent, '--');
tankApply({ v: 33, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(tankHost, 'widget-tank-label').textContent, '33 %');
tankApply({ v: 66, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(tankHost, 'widget-tank-label').textContent, '66 %');
assert.ok(Number(firstByClass(tankHost, 'widget-tank-fill').attrs.height) > 0);

const indicatorHost = new FakeElement('div');
const indicatorApply = test.createWidgetRenderer(
  { id: 'indicator', widget: 'indicator', data_type: 'BOOL', writable: false },
  indicatorHost,
);
indicatorApply(null);
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-label').textContent, '--');
indicatorApply({ v: true, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-label').textContent, 'ON');
indicatorApply({ v: false, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-label').textContent, 'OFF');
assert.strictEqual(firstByClass(indicatorHost, 'widget-indicator-dot').classList.contains('active'), false);

const toggleHost = new FakeElement('div');
const toggleApply = test.createWidgetRenderer(
  { id: 'toggle', widget: 'toggle', data_type: 'BOOL', writable: true },
  toggleHost,
);
toggleApply(null);
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-label').textContent, '--');
toggleApply({ v: true, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-label').textContent, 'ON');
toggleApply({ v: false, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-label').textContent, 'OFF');
assert.strictEqual(firstByClass(toggleHost, 'widget-toggle-control').disabled, true);

const sliderHost = new FakeElement('div');
const sliderApply = test.createWidgetRenderer(
  { id: 'slider', widget: 'slider', data_type: 'REAL', writable: true, min: 0, max: 100, unit: '%' },
  sliderHost,
);
sliderApply(null);
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-label').textContent, '--');
sliderApply({ v: 25.5, q: 'good', ts_ms: 1 });
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-label').textContent, '25.5 %');
sliderApply({ v: 60, q: 'stale', ts_ms: 2 });
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-label').textContent, '60 %');
assert.strictEqual(firstByClass(sliderHost, 'widget-slider-control').disabled, true);

const card = new FakeElement('article');
const cardValue = new FakeElement('div');
const seen = [];
test.state.cards = new Map([
  ['widget-1', { card, value: cardValue, apply: (entry) => seen.push(entry ? entry.v : null) }],
]);
test.applyValueDelta({
  connected: true,
  timestamp_ms: 10,
  values: { 'widget-1': { v: 3, q: 'good', ts_ms: 10 } },
});
assert.strictEqual(card.dataset.quality, 'good');
test.applyValueDelta({
  connected: true,
  timestamp_ms: 11,
  values: { 'widget-1': { v: 4, q: 'stale', ts_ms: 11 } },
});
assert.strictEqual(card.dataset.quality, 'stale');
test.applyValueDelta({
  connected: true,
  timestamp_ms: 12,
  values: { 'widget-1': null },
});
assert.strictEqual(card.dataset.quality, 'stale');
assert.deepStrictEqual(seen, [3, 4, null]);
assert.strictEqual(cardValue.classList.contains('value-updated'), true);
console.log('ok');
"#;
    run_node_hmi_script(&js_path, script, "widget renderer null/stale/good coverage");
}

#[test]
fn hmi_connector_summary_renders_shared_status_contract() {
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
}

class FakeElement {
  constructor(id) {
    this.id = id;
    this.textContent = '';
    this.title = '';
    this.classList = new ClassList();
  }
}

const elements = new Map();
for (const id of ['connectorSummaryState']) {
  elements.set(id, new FakeElement(id));
}

const noop = () => {};
const sourcePath = process.env.HMI_JS_PATH;
const source = fs.readFileSync(sourcePath, 'utf8') + '\n;globalThis.__hmi_test__ = { state, updateConnectorStatusSummary, markConnectorStatusUnavailable };';
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
    createElement: () => new FakeElement('node'),
    createElementNS: () => new FakeElement('node'),
    getElementById: (id) => elements.get(id) || null,
    body: { classList: new ClassList() },
    documentElement: { style: { setProperty: noop, removeProperty: noop } },
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
test.updateConnectorStatusSummary({
  connectors: [
    {
      state: 'ready',
      health: 'ok',
      confidence: 'confirmed',
      point_counts: { total: 3, good: 2, degraded: 1, unavailable: 0 },
    },
    {
      state: 'not_ready',
      health: 'unknown',
      confidence: 'port_reachable',
      point_counts: { total: 1, good: 0, degraded: 0, unavailable: 1 },
    },
  ],
});

assert.strictEqual(elements.get('connectorSummaryState').textContent, 'Connections: 1 ready, 1 needs attention');
assert.ok(elements.get('connectorSummaryState').classList.contains('stale'));
assert.ok(elements.get('connectorSummaryState').title.includes('Connection: 1 ready, 1 needs attention'));
assert.ok(elements.get('connectorSummaryState').title.includes('Verification: 1 confirmed, 1 port reachable only'));
assert.ok(elements.get('connectorSummaryState').title.includes('Signals: 2 good, 2 need attention'));
assert.ok(!elements.get('connectorSummaryState').textContent.includes('proof'));
assert.ok(!elements.get('connectorSummaryState').textContent.includes('tcp-only'));

test.markConnectorStatusUnavailable(new Error('control timeout'));
assert.strictEqual(elements.get('connectorSummaryState').textContent, 'Connections: unavailable');
assert.ok(elements.get('connectorSummaryState').title.includes('control timeout'));
console.log('ok');
"#;
    run_node_hmi_script(
        &js_path,
        script,
        "connector status summary contract rendering",
    );
}
