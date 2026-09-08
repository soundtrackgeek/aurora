import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";

const VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const ZERO_SHA = /^0+$/;

const readCurrentPackage = () => JSON.parse(fs.readFileSync("package.json", "utf8"));

const readPackageAtRef = (ref) => {
  if (!ref || ZERO_SHA.test(ref)) {
    return null;
  }

  try {
    const packageText = execFileSync("git", ["show", `${ref}:package.json`], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return JSON.parse(packageText);
  } catch {
    return null;
  }
};

const appendOutput = (values) => {
  if (!process.env.GITHUB_OUTPUT) {
    return;
  }

  const lines = Object.entries(values).map(([key, value]) => `${key}=${value ?? ""}`);
  fs.appendFileSync(process.env.GITHUB_OUTPUT, `${lines.join(os.EOL)}${os.EOL}`);
};

const currentPackage = readCurrentPackage();
const version = currentPackage.version;

if (!VERSION_PATTERN.test(version)) {
  throw new Error(`package.json version must be a valid semantic version. Received: ${version}`);
}

const previousPackage =
  readPackageAtRef(process.env.VERSION_BASE_REF) ??
  readPackageAtRef("HEAD^");

const previousVersion = previousPackage?.version ?? "";
const changed = previousVersion !== version;
const tag = `v${version}`;

appendOutput({
  changed: changed ? "true" : "false",
  previous_version: previousVersion,
  tag,
  version,
});

if (changed) {
  console.log(previousVersion ? `Version changed from ${previousVersion} to ${version}.` : `Version ${version} is new.`);
} else {
  console.log(`Version ${version} did not change.`);
}
