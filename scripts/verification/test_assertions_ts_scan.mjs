import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const request = JSON.parse(fs.readFileSync(0, "utf8"));
const root = path.resolve(request.root);
const typescriptPath = path.join(
  root,
  "editors",
  "vscode",
  "node_modules",
  "typescript"
);
const require = createRequire(import.meta.url);
const ts = require(typescriptPath);

const inventories = request.tests.map((test) => inventory(test));
process.stdout.write(
  JSON.stringify({ schema_version: 1, inventories }, null, 2)
);

function inventory(test) {
  const relative = normalizedRelative(test.path);
  const absolute = path.resolve(root, relative);
  if (!isWithin(absolute, root) || !fs.statSync(absolute).isFile()) {
    throw new Error(`TypeScript assertion source is invalid: ${relative}`);
  }
  const sourceText = fs.readFileSync(absolute, "utf8");
  const sourceFile = ts.createSourceFile(
    relative,
    sourceText,
    ts.ScriptTarget.Latest,
    true,
    relative.endsWith(".js") ? ts.ScriptKind.JS : ts.ScriptKind.TS
  );
  const assertionImports = importedAssertionNames(sourceFile);
  const matches = [];
  visit(sourceFile, (node) => {
    if (!ts.isCallExpression(node) || !isTestCall(node.expression)) {
      return;
    }
    const title = node.arguments[0];
    if (
      !title ||
      (!ts.isStringLiteral(title) && !ts.isNoSubstitutionTemplateLiteral(title)) ||
      title.text !== test.name
    ) {
      return;
    }
    const callback = node.arguments[1];
    if (
      callback &&
      (ts.isFunctionExpression(callback) || ts.isArrowFunction(callback))
    ) {
      matches.push(callback);
    }
  });
  if (matches.length !== 1) {
    throw new Error(
      `${relative}#${test.name} expected one literal test callback, got ${matches.length}`
    );
  }
  const assertions = [];
  const helpers = new Set();
  visit(matches[0].body, (node) => {
    if (!ts.isCallExpression(node)) {
      return;
    }
    const callee = node.expression.getText(sourceFile);
    const recognized =
      callee === "assert" ||
      callee.startsWith("assert.") ||
      callee === "expect" ||
      callee.startsWith("expect.") ||
      assertionImports.has(callee);
    if (!recognized) {
      if (/assert/i.test(callee)) {
        helpers.add(callee);
      }
      return;
    }
    const expression = node.getText(sourceFile).trim();
    const line =
      sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
    const expressionDigest = digest(expression);
    assertions.push({
      assertion_id: assertionId({
        path: relative,
        name: test.name,
        functionName: callee,
        kind: `call:${callee}`,
        line,
        expressionDigest,
      }),
      kind: `call:${callee}`,
      function: callee,
      line,
      expression_digest: expressionDigest,
      expression,
    });
  });
  assertions.sort((left, right) =>
    left.line - right.line ||
    left.function.localeCompare(right.function) ||
    left.assertion_id.localeCompare(right.assertion_id)
  );
  return {
    schema_version: 1,
    path: relative,
    test_name: test.name,
    source_digest: digest(sourceText),
    assertions,
    unresolved_assertion_helpers: [...helpers].sort(),
  };
}

function importedAssertionNames(sourceFile) {
  const names = new Set();
  for (const statement of sourceFile.statements) {
    if (!ts.isImportDeclaration(statement)) {
      continue;
    }
    const moduleName = statement.moduleSpecifier.text;
    if (moduleName !== "assert" && moduleName !== "node:assert") {
      continue;
    }
    const clause = statement.importClause;
    if (!clause) {
      continue;
    }
    if (clause.name) {
      names.add(clause.name.text);
    }
    if (clause.namedBindings && ts.isNamespaceImport(clause.namedBindings)) {
      names.add(clause.namedBindings.name.text);
    } else if (
      clause.namedBindings &&
      ts.isNamedImports(clause.namedBindings)
    ) {
      for (const element of clause.namedBindings.elements) {
        names.add(element.name.text);
      }
    }
  }
  return names;
}

function isTestCall(expression) {
  const text = expression.getText();
  return text === "test" || text === "it" || text === "test.skip" || text === "it.skip";
}

function visit(rootNode, callback) {
  callback(rootNode);
  rootNode.forEachChild((child) => visit(child, callback));
}

function assertionId({
  path: sourcePath,
  name,
  functionName,
  kind,
  line,
  expressionDigest,
}) {
  const identity = [
    sourcePath,
    name,
    functionName,
    kind,
    String(line),
    expressionDigest,
  ].join("\0");
  return `ASSERT_${crypto
    .createHash("sha256")
    .update(identity)
    .digest("hex")
    .slice(0, 24)
    .toUpperCase()}`;
}

function digest(value) {
  return `sha256:${crypto.createHash("sha256").update(value).digest("hex")}`;
}

function normalizedRelative(value) {
  if (typeof value !== "string" || value.length === 0 || path.isAbsolute(value)) {
    throw new Error("TypeScript assertion path must be repository-relative");
  }
  const normalized = value.replaceAll("\\", "/");
  if (normalized.split("/").includes("..")) {
    throw new Error("TypeScript assertion path escapes repository");
  }
  return normalized;
}

function isWithin(candidate, directory) {
  const relative = path.relative(path.resolve(directory), path.resolve(candidate));
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}
