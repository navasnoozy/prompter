import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { cwd, exit } from "node:process";

const root = cwd();
const packageJson = JSON.parse(
  readFileSync(resolve(root, "package.json"), "utf8"),
);
const packageLock = JSON.parse(
  readFileSync(resolve(root, "package-lock.json"), "utf8"),
);
const tauriConfig = JSON.parse(
  readFileSync(resolve(root, "src-tauri/tauri.conf.json"), "utf8"),
);
const cargoManifest = readFileSync(
  resolve(root, "src-tauri/Cargo.toml"),
  "utf8",
);
const rustToolchain = readFileSync(
  resolve(root, "rust-toolchain.toml"),
  "utf8",
);
const ciWorkflow = readFileSync(
  resolve(root, ".github/workflows/ci.yml"),
  "utf8",
);
const nodeVersion = readFileSync(resolve(root, ".nvmrc"), "utf8").trim();
const cargoVersion = cargoManifest.match(
  /^version\s*=\s*"([^"]+)"/m,
)?.[1];

const versions = new Map([
  ["package.json", packageJson.version],
  ["package-lock.json", packageLock.packages?.[""]?.version],
  ["tauri.conf.json", tauriConfig.version],
  ["Cargo.toml", cargoVersion],
]);
const unique = new Set(versions.values());

if (unique.size !== 1 || unique.has(undefined)) {
  for (const [file, version] of versions) {
    console.error(`${file}: ${version ?? "missing"}`);
  }
  exit(1);
}

const problems = [];
const nodeParts = nodeVersion.split(".");
const expectedNodeEngine = `${nodeParts[0]}.${nodeParts[1]}.x`;
if (nodeParts.length !== 3 || nodeParts.some((part) => !/^\d+$/.test(part))) {
  problems.push(`.nvmrc: invalid exact Node version ${nodeVersion}`);
}
if (packageJson.engines?.node !== expectedNodeEngine) {
  problems.push(
    `package.json engines.node: expected ${expectedNodeEngine}, received ${packageJson.engines?.node ?? "missing"}`,
  );
}
if (packageLock.packages?.[""]?.engines?.node !== packageJson.engines?.node) {
  problems.push("package-lock.json engines.node does not match package.json");
}

const packageManager = /^npm@(\d+\.\d+\.\d+)$/.exec(
  packageJson.packageManager ?? "",
)?.[1];
const expectedNpmEngine = packageManager
  ?.split(".")
  .slice(0, 2)
  .concat("x")
  .join(".");
if (!packageManager) {
  problems.push("package.json packageManager must pin an exact npm version");
} else if (packageJson.engines?.npm !== expectedNpmEngine) {
  problems.push(
    `package.json engines.npm: expected ${expectedNpmEngine}, received ${packageJson.engines?.npm ?? "missing"}`,
  );
}
if (packageLock.packages?.[""]?.engines?.npm !== packageJson.engines?.npm) {
  problems.push("package-lock.json engines.npm does not match package.json");
}

if (process.versions.node !== nodeVersion) {
  problems.push(
    `runtime Node: expected ${nodeVersion}, received ${process.versions.node}`,
  );
}
const runtimeNpmVersion = /^npm\/([^\s]+)/.exec(
  process.env.npm_config_user_agent ?? "",
)?.[1];
if (!packageManager || runtimeNpmVersion !== packageManager) {
  problems.push(
    `runtime npm: expected ${packageManager ?? "an exact configured version"}, received ${runtimeNpmVersion ?? "unknown"}`,
  );
}

const rustVersion = cargoManifest.match(
  /^rust-version\s*=\s*"([^"]+)"/m,
)?.[1];
const rustChannel = rustToolchain.match(/^channel\s*=\s*"([^"]+)"/m)?.[1];
if (!rustVersion || !rustChannel) {
  problems.push("Rust version is missing from Cargo.toml or rust-toolchain.toml");
} else if (!rustChannel.startsWith(`${rustVersion}.`)) {
  problems.push(
    `Rust toolchain ${rustChannel} does not match Cargo rust-version ${rustVersion}`,
  );
}

const ciNodeVersions = [
  ...ciWorkflow.matchAll(/^\s*node-version:\s*([^\s#]+)/gm),
].map((match) => match[1]);
if (
  ciNodeVersions.length === 0 ||
  ciNodeVersions.some((version) => version !== nodeVersion)
) {
  problems.push(`CI Node versions must all equal .nvmrc (${nodeVersion})`);
}
const ciRustToolchains = [
  ...ciWorkflow.matchAll(/^\s*toolchain:\s*([^\s#]+)/gm),
].map((match) => match[1]);
if (
  !rustChannel ||
  ciRustToolchains.length === 0 ||
  ciRustToolchains.some((toolchain) => toolchain !== rustChannel)
) {
  problems.push(
    `CI Rust toolchain must equal rust-toolchain.toml (${rustChannel ?? "missing"})`,
  );
}

const externalActions = [
  ...ciWorkflow.matchAll(/^\s*uses:\s*([^@\s]+)@([^\s#]+)/gm),
];
if (externalActions.length === 0) {
  problems.push("CI workflow does not declare any external actions");
}
for (const [, action, reference] of externalActions) {
  if (!/^[a-f0-9]{40}$/.test(reference)) {
    problems.push(`CI action ${action} must use a full immutable commit SHA`);
  }
}

if (problems.length > 0) {
  for (const problem of problems) console.error(problem);
  exit(1);
}

console.log(
  `Prompter ${packageJson.version}: verified Node ${nodeVersion}, npm ${packageManager}, and configured Rust ${rustChannel}.`,
);
