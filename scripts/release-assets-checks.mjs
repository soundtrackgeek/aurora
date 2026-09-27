import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { prepareAssets } from './prepare-release-assets.mjs';
import { mergeAssets } from './merge-release-assets.mjs';

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'release-assets-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const identity = { version: '1.2.3', tag: 'v1.2.3', repository: 'owner/app' };
  for (const platform of ['windows', 'macos']) {
    const bundleRoot = path.join(root, `bundle-${platform}`);
    fs.mkdirSync(bundleRoot);
    const archive = platform === 'windows' ? 'Test App_1.2.3_x64-setup.exe' : 'Test App.app.tar.gz';
    fs.writeFileSync(path.join(bundleRoot, archive), 'fixture artifact');
    fs.writeFileSync(path.join(bundleRoot, `${archive}.sig`), `signed-${platform}\n`);
    if (platform === 'macos') fs.writeFileSync(path.join(bundleRoot, 'Test App_1.2.3_universal.dmg'), 'fixture dmg');
    prepareAssets({ ...identity, platform, bundleRoot, outputDir: path.join(root, 'inputs', platform), notes: 'Release notes' });
  }
  return { ...identity, inputDir: path.join(root, 'inputs'), outputDir: path.join(root, 'output') };
}

test('assembles both Mac architectures and Windows before publishing one manifest', t => {
  const args = fixture(t);
  const result = mergeAssets(args);
  assert.equal(Object.keys(result.platforms).length, 6);
  assert.equal(result.platforms['darwin-aarch64'].url, result.platforms['darwin-x86_64'].url);
  assert.match(result.platforms['darwin-aarch64'].url, /1\.2\.3_universal\.app\.tar\.gz$/);
  assert.match(result.platforms['windows-x86_64'].url, /setup\.exe$/);
  assert.equal(result.platforms['darwin-aarch64-app'].signature, 'signed-macos');
  assert.ok(fs.readdirSync(args.outputDir).some(file => file.endsWith('.dmg')));
});

test('rejects a missing Mac architecture instead of publishing a Windows-only update', t => {
  const args = fixture(t);
  const file = path.join(args.inputDir, 'macos', 'manifest-macos.json');
  const manifest = JSON.parse(fs.readFileSync(file));
  delete manifest.platforms['darwin-x86_64'];
  fs.writeFileSync(file, JSON.stringify(manifest));
  assert.throws(() => mergeAssets(args), /Required updater platform missing/);
});

test('rejects mixed versions and signatures that do not match uploaded artifacts', t => {
  const args = fixture(t);
  const file = path.join(args.inputDir, 'macos', 'manifest-macos.json');
  const manifest = JSON.parse(fs.readFileSync(file));
  manifest.version = '1.2.2';
  fs.writeFileSync(file, JSON.stringify(manifest));
  assert.throws(() => mergeAssets(args), /versions or notes differ/);
  fs.rmSync(args.outputDir, { recursive: true });
  manifest.version = args.version;
  manifest.platforms['darwin-aarch64'].signature = 'wrong';
  fs.writeFileSync(file, JSON.stringify(manifest));
  assert.throws(() => mergeAssets(args), /signature is missing or mismatched/);
});

test('rejects updater URLs outside the intended release', t => {
  const args = fixture(t);
  const file = path.join(args.inputDir, 'macos', 'manifest-macos.json');
  const manifest = JSON.parse(fs.readFileSync(file));
  manifest.platforms['darwin-aarch64'].url = 'https://other.example/update';
  fs.writeFileSync(file, JSON.stringify(manifest));
  assert.throws(() => mergeAssets(args), /does not belong to this release/);
});

test('accepts Windows CRLF notes while rejecting genuinely different notes', t => {
  const args = fixture(t);
  for (const platform of ['windows', 'macos']) {
    const file = path.join(args.inputDir, platform, `manifest-${platform}.json`);
    const manifest = JSON.parse(fs.readFileSync(file));
    manifest.notes = platform === 'windows' ? 'Release notes\r\nSecond line' : 'Release notes\nSecond line';
    fs.writeFileSync(file, JSON.stringify(manifest));
  }
  assert.equal(mergeAssets(args).notes, 'Release notes\nSecond line');
  fs.rmSync(args.outputDir, { recursive: true });
  const file = path.join(args.inputDir, 'macos', 'manifest-macos.json');
  const manifest = JSON.parse(fs.readFileSync(file));
  manifest.notes += ' different content';
  fs.writeFileSync(file, JSON.stringify(manifest));
  assert.throws(() => mergeAssets(args), /versions or notes differ/);
});
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const notesScript = fileURLToPath(new URL('./extract-release-notes.mjs', import.meta.url));
function extractNotes(t, changelog, version = '1.2.3') {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'release-notes-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(root, 'CHANGELOG.md'), changelog);
  const result = spawnSync(process.execPath, [notesScript, version, 'notes.md'], { cwd: root, encoding: 'utf8' });
  return { ...result, notes: result.status === 0 ? fs.readFileSync(path.join(root, 'notes.md'), 'utf8') : null };
}

for (const current of ['[1.2.3]', '1.2.3']) {
  for (const previous of ['[1.2.2]', '1.2.2']) {
    test(`extracts ${current} and stops before ${previous} with LF or CRLF`, t => {
      for (const newline of ['\n', '\r\n']) {
        const result = extractNotes(t, ['# Changelog', '', `## ${current} - 2026-09-27`, '', '### Fixed', '- Current fix.', '', `## ${previous} - 2026-09-26`, '', '- Previous fix.'].join(newline));
        assert.equal(result.status, 0, result.stderr);
        assert.equal(result.notes, 'Release date: 2026-09-27\n\n### Fixed\n- Current fix.\n');
      }
    });
  }
}

test('rejects missing, empty, and partially bracketed release sections', t => {
  for (const changelog of ['## [1.2.2] - 2026-09-27\nOther', '## [1.2.3] - 2026-09-27\n\n## 1.2.2 - 2026-09-26\nOther', '## [1.2.3 - 2026-09-27\nBroken']) {
    const result = extractNotes(t, changelog);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /does not contain a release section|release section for .* is empty/);
  }
});

test('extracts the actual current release before installer assembly', t => {
  const { version } = JSON.parse(fs.readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  const result = extractNotes(t, fs.readFileSync(new URL('../CHANGELOG.md', import.meta.url), 'utf8'), version);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.notes, /^Release date: \d{4}-\d{2}-\d{2}\n\n/);
  assert.doesNotMatch(result.notes, /^## /m);
});
