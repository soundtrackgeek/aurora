import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
const root = new URL("../vendor/music-sonic-core/", import.meta.url);
const source = JSON.parse(readFileSync(new URL("SOURCE.json", root), "utf8"));
if (!/^[a-f0-9]{40}$/.test(source.revision)) throw new Error("Sonic contract requires an immutable upstream revision");
for (const file of ["Cargo.toml", "src/lib.rs", "LICENSE"]) {
  const content = readFileSync(new URL(file, root), "utf8").replaceAll("\r\n", "\n");
  const hash = createHash("sha256").update(content).digest("hex");
  if (hash !== source.sha256[file]) throw new Error(`Vendored sonic contract differs from its recorded source: ${file}`);
}
console.log(`Sonic contract matches the snapshot from ${source.revision.slice(0, 12)}.`);
