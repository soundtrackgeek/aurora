import fs from "node:fs";

const [, , version, outputPath = "release-notes.md"] = process.argv;

if (!version) {
  throw new Error("Usage: node scripts/extract-release-notes.mjs <version> [output-path]");
}

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const changelog = fs.readFileSync("CHANGELOG.md", "utf8");
const headingPattern = new RegExp(`^## \\[${escapeRegExp(version)}\\] - (\\d{4}-\\d{2}-\\d{2})\\s*$`, "m");
const headingMatch = changelog.match(headingPattern);

if (!headingMatch || headingMatch.index === undefined) {
  throw new Error(`CHANGELOG.md does not contain a release section for ${version}.`);
}

const bodyStart = headingMatch.index + headingMatch[0].length;
const remaining = changelog.slice(bodyStart);
const nextHeadingIndex = remaining.search(/^## \[/m);
const sectionBody = (nextHeadingIndex >= 0 ? remaining.slice(0, nextHeadingIndex) : remaining).trim();

if (!sectionBody) {
  throw new Error(`CHANGELOG.md release section for ${version} is empty.`);
}

const notes = [`Release date: ${headingMatch[1]}`, "", sectionBody, ""].join("\n");
fs.writeFileSync(outputPath, notes, "utf8");
console.log(`Wrote release notes for ${version} to ${outputPath}.`);
