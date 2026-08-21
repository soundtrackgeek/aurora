import { readFileSync } from "node:fs";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const tauriConfig = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const cargoToml = readFileSync(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const versions = new Map([
  ["package.json", packageJson.version],
  ["src-tauri/tauri.conf.json", tauriConfig.version],
  ["src-tauri/Cargo.toml", cargoVersion],
]);
const unique = new Set(versions.values());

if (unique.size !== 1 || unique.has(undefined)) {
  console.error("Aurora version mismatch:");
  for (const [file, version] of versions) console.error(`- ${file}: ${version ?? "missing"}`);
  process.exit(1);
}

const version = packageJson.version;
if (process.env.GITHUB_REF_TYPE === "tag" && process.env.GITHUB_REF_NAME !== `v${version}`) {
  console.error(`Release tag ${process.env.GITHUB_REF_NAME} does not match Aurora ${version}.`);
  process.exit(1);
}

console.log(`Aurora versions aligned at ${version}.`);
