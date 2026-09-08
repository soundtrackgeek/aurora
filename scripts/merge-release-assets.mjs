import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export function mergeAssets({ inputDir, outputDir, version, tag, repository }) {
  const platforms = {};
  let notes;
  fs.mkdirSync(outputDir, { recursive: true });
  for (const platform of ['windows', 'macos']) {
    const folder = path.join(inputDir, platform);
    const manifest = JSON.parse(fs.readFileSync(path.join(folder, `manifest-${platform}.json`), 'utf8'));
    if (manifest.version !== version || (notes !== undefined && notes !== manifest.notes)) throw new Error('Release versions or notes differ');
    notes = manifest.notes;
    for (const file of fs.readdirSync(folder)) {
      if (file.startsWith('manifest-')) continue;
      if (fs.existsSync(path.join(outputDir, file))) throw new Error(`Duplicate release asset: ${file}`);
      fs.copyFileSync(path.join(folder, file), path.join(outputDir, file));
    }
    for (const [key, entry] of Object.entries(manifest.platforms)) {
      if (platforms[key]) throw new Error(`Duplicate platform: ${key}`);
      const prefix = `https://github.com/${repository}/releases/download/${tag}/`;
      if (!entry.url.startsWith(prefix)) throw new Error('Updater URL does not belong to this release');
      const file = decodeURIComponent(entry.url.slice(prefix.length));
      if (path.basename(file) !== file || file.includes('\\') || !fs.existsSync(path.join(outputDir, file))) throw new Error('Updater artifact is missing');
      if (!entry.signature || fs.readFileSync(path.join(outputDir, `${file}.sig`), 'utf8').trim() !== entry.signature) throw new Error('Updater signature is missing or mismatched');
      platforms[key] = entry;
    }
  }
  for (const key of ['windows-x86_64', 'windows-x86_64-nsis', 'darwin-aarch64', 'darwin-aarch64-app', 'darwin-x86_64', 'darwin-x86_64-app']) {
    if (!platforms[key]) throw new Error(`Required updater platform missing: ${key}`);
  }
  const result = { version, notes, pub_date: new Date().toISOString(), platforms };
  fs.writeFileSync(path.join(outputDir, 'latest.json'), `${JSON.stringify(result, null, 2)}\n`);
  return result;
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const [, , version, tag, repository] = process.argv;
  mergeAssets({ inputDir: 'release-inputs', outputDir: 'release-assets', version, tag, repository });
}
