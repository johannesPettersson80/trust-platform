"use strict";

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

function digest(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function normalized(file) {
  return path.resolve(String(file || "")).toLowerCase();
}

function provePackagedBinaryIdentity({
  extensionRoot,
  packageProof,
  productIdentity,
  check,
}) {
  const expected = {
    language_server: path.join(extensionRoot, "bin", "trust-lsp.exe"),
    debug_adapter: path.join(extensionRoot, "bin", "trust-debug.exe"),
    runtime: path.join(extensionRoot, "bin", "trust-runtime.exe"),
  };
  const configured = {
    language_server: productIdentity?.binaries?.languageServer || "",
    debug_adapter: productIdentity?.binaries?.debugAdapter || "",
    runtime: productIdentity?.binaries?.runtime || "",
  };
  const pathFallbackBlocked =
    process.env.TRUST_PACKAGED_PATH_FALLBACK_BLOCKED === "1";
  const exact = Object.keys(expected).every(
    (key) =>
      fs.existsSync(expected[key]) &&
      normalized(configured[key]) === normalized(expected[key])
  );
  packageProof.extension_js_sha256 = digest(
    path.join(extensionRoot, "out", "extension.js")
  );
  packageProof.runtime_sha256 = digest(expected.runtime);
  packageProof.debug_sha256 = digest(expected.debug_adapter);
  packageProof.lsp_sha256 = digest(expected.language_server);
  packageProof.binary_resolution = {
    mode: "explicit-isolated-installed-settings",
    configured,
    product_extension_mode: productIdentity?.extensionMode,
    exact_installed_paths: exact,
    path_fallback_blocked: pathFallbackBlocked,
  };
  check("exact-packaged-binaries-selected", exact && pathFallbackBlocked, {
    mode: packageProof.binary_resolution.mode,
    exact_installed_paths: exact,
    path_fallback_blocked: pathFallbackBlocked,
  });
}

async function provePackagedProductIdentity({
  vscode,
  extension,
  extensionRoot,
  expectedVersion,
  packageProof,
  check,
}) {
  const extensionApi = await extension.activate();
  const productIdentity = extensionApi?.getProductRuntimeIdentity?.();
  check(
    "packaged-extension-production-mode",
    productIdentity?.extensionMode === vscode.ExtensionMode.Production &&
      normalized(productIdentity?.extensionPath) === normalized(extensionRoot) &&
      productIdentity?.extensionVersion === expectedVersion,
    {
      extension_mode: productIdentity?.extensionMode,
      product_identity_schema: productIdentity?.schemaVersion,
    }
  );
  provePackagedBinaryIdentity({
    extensionRoot,
    packageProof,
    productIdentity,
    check,
  });
}

module.exports = { provePackagedBinaryIdentity, provePackagedProductIdentity };
