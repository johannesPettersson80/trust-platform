#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

let input = "";
for await (const chunk of process.stdin) input += chunk;

const diagnostics = [];
const sources = [];
let request;
try {
  request = JSON.parse(input);
} catch (error) {
  emit([], [`invalid scanner request JSON: ${error.message}`]);
  process.exitCode = 1;
}

if (request) {
  const ts = loadTypeScript(request.root);
  if (!ts) {
    emit([], ["lockfile-pinned TypeScript compiler API is unavailable"]);
    process.exitCode = 1;
  } else if (request.schema_version !== 1 || !Array.isArray(request.files)) {
    emit([], ["scanner request must use schema_version 1 and a files array"]);
    process.exitCode = 1;
  } else {
    for (const file of request.files) scanFile(ts, request.root, file, sources, diagnostics);
    emit(sources, diagnostics);
    if (diagnostics.length) process.exitCode = 1;
  }
}

function loadTypeScript(root) {
  const candidates = [
    path.join(process.cwd(), "editors/vscode/node_modules/typescript"),
    typeof root === "string" ? path.join(root, "editors/vscode/node_modules/typescript") : "",
    typeof root === "string" ? path.join(root, "crates/trust-runtime/web/ide-frontend/node_modules/typescript") : "",
  ].filter(Boolean);
  for (const candidate of candidates) {
    try {
      return require(candidate);
    } catch (_error) {
      // Continue to the next reviewed lockfile installation.
    }
  }
  return null;
}

function scanFile(ts, root, file, output, allDiagnostics) {
  if (!file || typeof file.path !== "string" || typeof file.module !== "string") {
    allDiagnostics.push("scanner file record requires path and module strings");
    return;
  }
  if (!new Set(["typescript", "javascript"]).has(file.source_kind)) {
    allDiagnostics.push(`${file.path}: unsupported source kind ${file.source_kind}`);
    return;
  }
  if (!new Set(["ast", "asset"]).has(file.scan_mode)) {
    allDiagnostics.push(`${file.path}: unsupported scan mode ${file.scan_mode}`);
    return;
  }
  const normalized = file.path.replaceAll("\\", "/");
  if (path.isAbsolute(normalized) || normalized.split("/").includes("..")) {
    allDiagnostics.push(`${file.path}: source path must stay repository-relative`);
    return;
  }
  const absolute = path.resolve(root, normalized);
  const canonicalRoot = fs.realpathSync(root);
  let canonical;
  try {
    canonical = fs.realpathSync(absolute);
  } catch (error) {
    allDiagnostics.push(`${normalized}: unreadable source: ${error.message}`);
    return;
  }
  if (canonical !== canonicalRoot && !canonical.startsWith(canonicalRoot + path.sep)) {
    allDiagnostics.push(`${normalized}: source escapes repository root`);
    return;
  }
  const text = fs.readFileSync(canonical, "utf8");
  const scriptKind = file.source_kind === "javascript" ? ts.ScriptKind.JS : ts.ScriptKind.TS;
  const sourceFile = ts.createSourceFile(normalized, text, ts.ScriptTarget.Latest, true, scriptKind);
  if (sourceFile.parseDiagnostics.length) {
    for (const diagnostic of sourceFile.parseDiagnostics) {
      const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, " ");
      const line = diagnostic.start === undefined
        ? 1
        : sourceFile.getLineAndCharacterOfPosition(diagnostic.start).line + 1;
      allDiagnostics.push(`${normalized}:${line}: unsupported syntax: ${message}`);
    }
    return;
  }
  const facts = [];
  let registrationOrdinal = 0;
  const prefix = file.source_kind === "javascript" ? "javascript" : "typescript";

  function addFact(node, symbol, itemKind, signature, visibility = "private") {
    const start = node.getStart(sourceFile, false);
    const end = node.end;
    facts.push({
      qualified_symbol: symbol,
      item_kind: `${prefix}_${itemKind}`,
      visibility,
      signature: normalize(signature),
      body_start: Buffer.byteLength(text.slice(0, start), "utf8"),
      body_end: Buffer.byteLength(text.slice(0, end), "utf8"),
      line: sourceFile.getLineAndCharacterOfPosition(start).line + 1,
      risk_signals: riskSignals(node, ts, sourceFile),
    });
  }

  function qualified(context, name) {
    return [...context, name].filter(Boolean).join("::");
  }

  if (file.scan_mode === "asset") {
    facts.push({
      qualified_symbol: `${file.module}::<asset>`,
      item_kind: `${prefix}_asset`,
      visibility: "public",
      signature: "asset",
      body_start: 0,
      body_end: Buffer.byteLength(text, "utf8"),
      line: 1,
      risk_signals: [],
    });
    output.push({
      path: normalized,
      module: file.module,
      source_kind: file.source_kind,
      scan_mode: file.scan_mode,
      facts,
    });
    return;
  }

  function visit(node, context) {
    if (ts.isFunctionDeclaration(node) && node.name && node.body) {
      const symbol = qualified(context, node.name.text);
      addFact(node, symbol, "function", callableSignature(node, sourceFile), exported(node) ? "public" : "private");
      ts.forEachChild(node, (child) => visit(child, [...context, node.name.text]));
      return;
    }
    if (ts.isClassDeclaration(node) && node.name) {
      if (exported(node)) {
        addFact(node, qualified(context, node.name.text), "export", declarationHeader(node, sourceFile), "public");
      }
      ts.forEachChild(node, (child) => visit(child, [...context, node.name.text]));
      return;
    }
    if (ts.isClassExpression(node)) {
      const classContext = node.name ? [...context, node.name.text] : context;
      ts.forEachChild(node, (child) => visit(child, classContext));
      return;
    }
    if (ts.isMethodDeclaration(node) && node.name && node.body) {
      const name = propertyName(node.name, sourceFile);
      const symbol = qualified(context, name);
      addFact(node, symbol, "method", callableSignature(node, sourceFile), visibility(node, ts));
      ts.forEachChild(node, (child) => visit(child, [...context, name]));
      return;
    }
    if (ts.isPropertyDeclaration(node) && node.name && node.initializer
        && (ts.isArrowFunction(node.initializer) || ts.isFunctionExpression(node.initializer))) {
      const name = propertyName(node.name, sourceFile);
      addFact(node, qualified(context, name), "method", callableSignature(node.initializer, sourceFile), visibility(node, ts));
    }
    if (ts.isVariableStatement(node)) {
      for (const declaration of node.declarationList.declarations) {
        if (!ts.isIdentifier(declaration.name)) {
          ts.forEachChild(declaration, (child) => visit(child, context));
          continue;
        }
        const name = declaration.name.text;
        if (declaration.initializer
            && (ts.isArrowFunction(declaration.initializer) || ts.isFunctionExpression(declaration.initializer))) {
          addFact(declaration.initializer, qualified(context, name), "function", callableSignature(declaration.initializer, sourceFile), exported(node) ? "public" : "private");
        } else if (exported(node)) {
          addFact(declaration, qualified(context, name), "export", declaration.getText(sourceFile), "public");
        }
        if (declaration.initializer) visit(declaration.initializer, [...context, name]);
      }
      return;
    }
    if (ts.isPropertyAssignment(node)) {
      const name = propertyName(node.name, sourceFile);
      visit(node.initializer, [...context, name]);
      return;
    }
    if (ts.isFunctionExpression(node) || ts.isArrowFunction(node)) {
      const functionContext = ts.isFunctionExpression(node) && node.name
        ? [...context, node.name.text]
        : context;
      ts.forEachChild(node, (child) => visit(child, functionContext));
      return;
    }
    if ((ts.isInterfaceDeclaration(node) || ts.isTypeAliasDeclaration(node) || ts.isEnumDeclaration(node))
        && node.name && exported(node)) {
      addFact(node, qualified(context, node.name.text), "export", node.getText(sourceFile), "public");
    }
    if (ts.isCallExpression(node)) {
      const api = registrationApi(node.expression, ts);
      if (api) {
        registrationOrdinal += 1;
        const key = literalKey(node.arguments[0], ts) ?? api;
        const parent = context.join("::");
        addFact(
          node,
          `${parent}::${api}[${registrationOrdinal}:${key}]`,
          "registration",
          node.getText(sourceFile),
        );
      }
    }
    ts.forEachChild(node, (child) => visit(child, context));
  }

  visit(sourceFile, [file.module]);
  facts.sort((left, right) =>
    left.qualified_symbol.localeCompare(right.qualified_symbol)
    || left.item_kind.localeCompare(right.item_kind)
    || left.body_start - right.body_start
  );
  output.push({
    path: normalized,
    module: file.module,
    source_kind: file.source_kind,
    scan_mode: file.scan_mode,
    facts,
  });
}

function registrationApi(expression, ts) {
  const APIs = new Set([
    "register",
    "registerCommand",
    "registerTextEditorCommand",
    "registerWebviewViewProvider",
    "onDidReceiveMessage",
    "onRequest",
    "onNotification",
    "addEventListener",
  ]);
  if (ts.isPropertyAccessExpression(expression) && APIs.has(expression.name.text)) return expression.name.text;
  if (ts.isIdentifier(expression) && APIs.has(expression.text)) return expression.text;
  return null;
}

function literalKey(argument, ts) {
  if (!argument) return null;
  if (ts.isStringLiteralLike(argument) || ts.isNumericLiteral(argument)) return argument.text;
  return null;
}

function propertyName(name, sourceFile) {
  return name.text ?? name.getText(sourceFile);
}

function callableSignature(node, sourceFile) {
  if (node.body) {
    const start = node.getStart(sourceFile, false);
    const bodyStart = node.body.getStart(sourceFile, false);
    return sourceFile.text.slice(start, bodyStart).trim();
  }
  return node.getText(sourceFile);
}

function declarationHeader(node, sourceFile) {
  const start = node.getStart(sourceFile, false);
  const brace = sourceFile.text.indexOf("{", start);
  return brace >= start && brace < node.end
    ? sourceFile.text.slice(start, brace).trim()
    : node.getText(sourceFile);
}

function exported(node) {
  return Boolean(node.modifiers?.some((modifier) => modifier.kind === 95 || modifier.getText() === "export"));
}

function visibility(node, ts) {
  return node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.PublicKeyword) ? "public" : "private";
}

function normalize(value) {
  return value.split(/\s+/u).filter(Boolean).join(" ");
}

function riskSignals(rootNode, ts, sourceFile) {
  const signals = new Set();
  function visit(node) {
    if (node !== rootNode && ts.isFunctionLike(node)) return;
    if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
      addRisksForName(node.getText(sourceFile), signals);
    }
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      addRisksForName(node.getText(sourceFile), signals);
    }
    if (ts.isBinaryExpression(node)
        && node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment
        && node.operatorToken.kind <= ts.SyntaxKind.LastAssignment) {
      signals.add("state_mutation");
    }
    if (ts.isCatchClause(node)
        && (node.block.statements.length === 0 || node.variableDeclaration?.name?.getText(sourceFile).startsWith("_"))) {
      signals.add("broad_error_suppression");
    }
    ts.forEachChild(node, visit);
  }
  visit(rootNode);
  return [...signals].sort();
}

function addRisksForName(name, signals) {
  const normalized = name.toLowerCase();
  const policies = [
    ["host_time", ["date.now", "new date(", "performance.now"]],
    ["randomness", ["math.random", "crypto.getrandomvalues", "randomuuid"]],
    ["filesystem_or_environment", ["process.env", "localstorage", "sessionstorage", "filesystem", "readfile", "writefile"]],
    ["network_io", ["fetch(", "websocket", "xmlhttprequest", "sendbeacon"]],
    ["subprocess_or_thread", ["child_process", "worker(", "workerthread", "spawn("]],
    ["state_mutation", [".set(", ".add(", ".delete(", ".push(", ".remove(", "appendchild", "replacechildren"]],
    ["output_or_safe_state_commit", ["safe_state", "forceoutput", "releaseoutput", "commitoutput"]],
    ["authorization", ["authorize", "authorization", "authz", "permission", "role"]],
    ["fault_mapping", ["maperror", "fault", "errorresponse"]],
    ["protocol_response_or_status", ["response", "status", "health"]],
    ["broad_error_suppression", ["catch(()", ".catch(_", "ignoreerror"]],
    ["persistence_retain_reload_restart", ["persist", "retain", "reload", "restart"]],
  ];
  for (const [signal, needles] of policies) {
    if (needles.some((needle) => normalized.includes(needle))) signals.add(signal);
  }
}

function emit(scannedSources, emittedDiagnostics) {
  process.stdout.write(JSON.stringify({
    schema_version: 1,
    sources: scannedSources,
    diagnostics: [...new Set(emittedDiagnostics)].sort(),
  }));
}
