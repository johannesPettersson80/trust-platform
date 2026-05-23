
const STATE_KEY = "__scenaOutputColorSpace";

function baseState(backend, requested) {
  return {
    api: backend,
    property: backend === "webgpu"
      ? "GPUCanvasConfiguration.colorSpace"
      : "drawingBufferColorSpace",
    requested,
    supported: false,
    configured: false,
    effective: backend === "webgpu" ? null : "srgb",
    display_p3: false,
    injected_by: "RendererOptions::with_output_color_space",
    error: null,
  };
}

function installWebGpuDisplayP3Hook(canvas, requested) {
  const state = baseState("webgpu", requested);
  const context = canvas.getContext("webgpu");
  if (!context || typeof context.configure !== "function") {
    state.error = "webgpu context unavailable";
    canvas[STATE_KEY] = state;
    return state;
  }
  if (context.__scenaDisplayP3HookInstalled) {
    const existing = context[STATE_KEY] || canvas[STATE_KEY] || state;
    canvas[STATE_KEY] = existing;
    return existing;
  }
  const original = context.configure.bind(context);
  const hooked = function(config) {
    const displayP3Config = Object.assign({}, config, { colorSpace: "display-p3" });
    try {
      original(displayP3Config);
      const configured = baseState("webgpu", "display-p3");
      configured.supported = true;
      configured.configured = true;
      configured.effective = "display-p3";
      configured.display_p3 = true;
      context[STATE_KEY] = configured;
      canvas[STATE_KEY] = configured;
    } catch (caught) {
      original(config);
      const fallback = baseState("webgpu", "display-p3");
      fallback.supported = false;
      fallback.configured = false;
      fallback.effective = null;
      fallback.display_p3 = false;
      fallback.error = String(caught && caught.message ? caught.message : caught);
      context[STATE_KEY] = fallback;
      canvas[STATE_KEY] = fallback;
    }
  };
  try {
    context.configure = hooked;
  } catch (caught) {
    try {
      Object.defineProperty(context, "configure", { value: hooked, configurable: true });
    } catch (defineCaught) {
      state.error = String(defineCaught && defineCaught.message ? defineCaught.message : caught);
      canvas[STATE_KEY] = state;
      return state;
    }
  }
  context.__scenaDisplayP3HookInstalled = true;
  state.supported = true;
  state.effective = "pending-configure";
  canvas[STATE_KEY] = state;
  return state;
}

function configureWebGl2DisplayP3(canvas, requested) {
  const state = baseState("webgl2", requested);
  const gl = canvas.getContext("webgl2");
  if (!gl) {
    state.error = "webgl2 context unavailable";
    canvas[STATE_KEY] = state;
    return state;
  }
  state.supported = "drawingBufferColorSpace" in gl;
  if (!state.supported) {
    state.error = "drawingBufferColorSpace unavailable";
    canvas[STATE_KEY] = state;
    return state;
  }
  try {
    if (requested === "display-p3") {
      gl.drawingBufferColorSpace = "display-p3";
    }
    state.effective = gl.drawingBufferColorSpace || null;
    state.configured = state.effective === requested;
    state.display_p3 = state.effective === "display-p3";
  } catch (caught) {
    state.error = String(caught && caught.message ? caught.message : caught);
  }
  canvas[STATE_KEY] = state;
  return state;
}

export function scenaPrepareBrowserCanvasOutputColorSpace(canvas, backend, requested) {
  if (requested !== "display-p3") {
    const state = baseState(backend, requested);
    state.supported = true;
    state.configured = true;
    state.effective = "srgb";
    canvas[STATE_KEY] = state;
    return state;
  }
  if (backend === "webgpu") {
    return installWebGpuDisplayP3Hook(canvas, requested);
  }
  return baseState(backend, requested);
}

export function scenaRefreshBrowserCanvasOutputColorSpace(canvas, backend, requested) {
  if (requested !== "display-p3") {
    return scenaPrepareBrowserCanvasOutputColorSpace(canvas, backend, requested);
  }
  if (backend === "webgl2") {
    return configureWebGl2DisplayP3(canvas, requested);
  }
  const context = canvas.getContext("webgpu");
  const state = (context && context[STATE_KEY]) || canvas[STATE_KEY] || baseState("webgpu", requested);
  canvas[STATE_KEY] = state;
  return state;
}
