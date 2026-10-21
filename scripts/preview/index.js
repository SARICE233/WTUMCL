const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const ROOT = path.join(__dirname, "../..");
const PREVIEW_DIR = path.join(ROOT, "preview");
const PREVIEW_EXE = path.join(PREVIEW_DIR, "WTUMCL-dev.exe");
const DEBUG_EXE = path.join(ROOT, "src-tauri/target/debug/WTUMCL.exe");
const RELEASE_EXE = path.join(ROOT, "src-tauri/target/release/WTUMCL.exe");
const NSIS_DIR = path.join(ROOT, "src-tauri/target/release/bundle/nsis");

const args = process.argv.slice(2);
const knownArgs = new Set(["--skip-build", "--list"]);
const unknownArgs = args.filter((arg) => !knownArgs.has(arg));
if (unknownArgs.length > 0) {
  console.error(`Unknown argument(s): ${unknownArgs.join(", ")}`);
  console.error("Usage: node scripts/preview/index.js [--skip-build | --list]");
  process.exit(1);
}

function formatSize(bytes) {
  if (bytes < 1024 * 1024) {
    return `${bytes} bytes`;
  }
  return `${bytes} bytes (${(bytes / (1024 * 1024)).toFixed(1)} MB)`;
}

function describeFile(filePath) {
  try {
    const stat = fs.statSync(filePath);
    if (!stat.isFile()) {
      return `${filePath} | MISSING`;
    }
    return `${filePath} | ${formatSize(stat.size)} | ${stat.mtime.toISOString()}`;
  } catch {
    return `${filePath} | MISSING`;
  }
}

function findNsisInstallers() {
  try {
    return fs
      .readdirSync(NSIS_DIR)
      .filter((name) => /setup\.exe$/i.test(name))
      .map((name) => path.join(NSIS_DIR, name));
  } catch {
    return [];
  }
}

function printArtifactList() {
  console.log("WTUMCL preview artifacts:");
  console.log(`- preview:           ${describeFile(PREVIEW_EXE)}`);
  console.log(`- debug binary:      ${describeFile(DEBUG_EXE)}`);
  console.log(`- release binary:    ${describeFile(RELEASE_EXE)}`);

  const installers = findNsisInstallers();
  if (installers.length === 0) {
    console.log(
      `- nsis installer:    ${path.join(NSIS_DIR, "*setup.exe")} | MISSING`
    );
  } else {
    for (const installer of installers) {
      console.log(`- nsis installer:    ${describeFile(installer)}`);
    }
  }
}

function copyPreview() {
  if (!fs.existsSync(DEBUG_EXE)) {
    console.error(`Debug binary not found: ${DEBUG_EXE}`);
    console.error("Run without --skip-build to build it first.");
    process.exit(1);
  }

  fs.mkdirSync(PREVIEW_DIR, { recursive: true });
  fs.copyFileSync(DEBUG_EXE, PREVIEW_EXE);

  console.log("Preview build ready:");
  console.log(`- artifact: ${describeFile(PREVIEW_EXE)}`);
  console.log(`- source:   ${describeFile(DEBUG_EXE)}`);
}

if (args.includes("--list")) {
  printArtifactList();
  process.exit(0);
}

if (args.includes("--skip-build")) {
  copyPreview();
  process.exit(0);
}

const result = spawnSync(
  "pnpm",
  [
    "exec",
    "tauri",
    "build",
    "--debug",
    "--no-bundle",
    "--features",
    "devtools",
  ],
  {
    cwd: ROOT,
    stdio: "inherit",
    shell: process.platform === "win32",
  }
);

if (result.error) {
  console.error(`Failed to start the build: ${result.error.message}`);
  process.exit(1);
}

if (result.status !== 0) {
  console.error(`Build failed with exit code ${result.status}`);
  process.exit(result.status === null ? 1 : result.status);
}

copyPreview();
