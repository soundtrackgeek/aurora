import { readFileSync } from "node:fs";
import { deepStrictEqual, ok } from "node:assert/strict";

// Compare snapshots produced by the opt-in sonic_performance_proof native test.
const [beforePath, afterPath] = process.argv.slice(2);
if (!beforePath || !afterPath) throw new Error("Usage: node scripts/compare-sonic-performance.mjs before.json after.json");
function read(path) {
  const json = JSON.parse(readFileSync(path, "utf8"));
  const records = Array.isArray(json) ? json : json.records;
  ok(Array.isArray(records) && records.length > 0, "Missing benchmark records");
  for (const record of records) {
    ok(Number.isInteger(record.iteration) && record.iteration >= 0);
    ok(Number.isFinite(record.ms) && record.ms > 0);
    ok(record.result && typeof record.task === "string");
  }
  return records;
}
const before = read(beforePath), after = read(afterPath);
deepStrictEqual(after.map(r => [r.task, r.iteration]), before.map(r => [r.task, r.iteration]), "Benchmark cases differ");
for (let i = 0; i < before.length; i++) {
  deepStrictEqual(after[i].result, before[i].result, `Changed ${before[i].task} response in iteration ${before[i].iteration}`);
}
function median(values) {
  values.sort((a, b) => a - b);
  const mid = Math.floor(values.length / 2);
  return values.length % 2 ? values[mid] : (values[mid - 1] + values[mid]) / 2;
}
for (const task of new Set(before.map(r => r.task))) {
  const cases = records => records.filter(r => r.task === task);
  // Iteration zero warms filesystem/SQLite caches and is reported separately.
  const warm = records => cases(records).filter(r => r.iteration > 0).map(r => r.ms);
  ok(warm(before).length >= 2 && warm(after).length >= 2, "Need at least two warm samples per task");
  for (const records of [before, after]) {
    for (const record of cases(records)) deepStrictEqual(record.result, cases(records)[0].result, `Unstable ${task} responses`);
  }
  const baseline = median(warm(before)), optimized = median(warm(after));
  console.log(`${task}: ${baseline.toFixed(2)} -> ${optimized.toFixed(2)} ms warm median (${(baseline / optimized).toFixed(2)}x); identical responses`);
  console.log(`  first pass: ${cases(before)[0].ms.toFixed(2)} -> ${cases(after)[0].ms.toFixed(2)} ms`);
}
