const fs = require("fs");
const path = require("path");
const ts = require("typescript");

const extensionRoot = path.resolve(__dirname, "..");
const repositoryRoot = path.resolve(extensionRoot, "..", "..");
const sourceRoot = process.env.TRUST_EXAMPLES_SOURCE_ROOT
  ? path.resolve(process.env.TRUST_EXAMPLES_SOURCE_ROOT)
  : path.join(repositoryRoot, "examples");
const catalogPath = path.join(sourceRoot, "catalog.json");
const outputRoot = process.env.TRUST_EXAMPLES_OUTPUT_ROOT
  ? path.resolve(process.env.TRUST_EXAMPLES_OUTPUT_ROOT)
  : path.join(extensionRoot, "media", "examples");

const catalogText = fs.readFileSync(catalogPath, "utf8");
const entries = JSON.parse(catalogText);
if (!Array.isArray(entries)) {
  throw new Error("examples/catalog.json must contain an array");
}

const ids = new Set();
const paths = new Set();
const controlLessons = [];
const lessonValidator = loadLessonValidator();
for (const [index, entry] of entries.entries()) {
  for (const field of ["id", "title", "description", "path", "hardware"]) {
    if (typeof entry[field] !== "string" || !entry[field].trim()) {
      throw new Error(`catalog entry ${index} is missing '${field}'`);
    }
  }
  const relative = path.normalize(entry.path);
  if (
    path.isAbsolute(entry.path) ||
    relative === ".." ||
    relative.startsWith(`..${path.sep}`)
  ) {
    throw new Error(`catalog entry ${index} has an unsafe path '${entry.path}'`);
  }
  if (ids.has(entry.id) || paths.has(relative)) {
    throw new Error(`catalog entry ${index} duplicates an id or path`);
  }
  ids.add(entry.id);
  paths.add(relative);
  if (!fs.statSync(path.join(sourceRoot, relative)).isDirectory()) {
    throw new Error(`catalog entry ${index} is not a directory: ${entry.path}`);
  }
  if (entry.id.startsWith("control-") || (Array.isArray(entry.tags) && entry.tags.includes("control"))) {
    const lessonPath = path.join(sourceRoot, relative, "lesson.json");
    if (!fs.existsSync(lessonPath)) {
      throw new Error(`control lesson '${entry.id}' is missing lesson.json`);
    }
    const lesson = lessonValidator.parseControlLessonContract(
      JSON.parse(fs.readFileSync(lessonPath, "utf8")), entry.id
    );
    controlLessons.push({ id: entry.id, sequence: lesson.sequence, prerequisites: lesson.prerequisites });
  }
}

const curriculum = lessonValidator.validateControlCurriculum(controlLessons);
if (curriculum.kind !== "accepted") {
  throw new Error(`control curriculum is invalid: ${JSON.stringify(curriculum)}`);
}

// media/examples is package output. Maintainers edit only examples/ so the
// repository, docs, tests, and extension cannot acquire competing copies.
fs.rmSync(outputRoot, { recursive: true, force: true });
fs.mkdirSync(outputRoot, { recursive: true });
for (const entry of entries) {
  fs.cpSync(
    path.join(sourceRoot, entry.path),
    path.join(outputRoot, entry.path),
    { recursive: true, errorOnExist: true }
  );
}
fs.writeFileSync(path.join(outputRoot, "manifest.json"), catalogText);

console.log(`Bundled ${entries.length} examples from examples/catalog.json`);

/** Execute the one TypeScript validator source; packaging does not own a copy. */
function loadLessonValidator() {
  const source = fs.readFileSync(path.join(extensionRoot, "src", "examples", "controlLesson.ts"), "utf8");
  const javascript = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS, strict: true },
    fileName: "controlLesson.ts",
  }).outputText;
  const validatorModule = { exports: {} };
  const execute = new Function("module", "exports", "require", javascript);
  execute(validatorModule, validatorModule.exports, require);
  return validatorModule.exports;
}
