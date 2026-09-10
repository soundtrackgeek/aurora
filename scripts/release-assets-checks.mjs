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
