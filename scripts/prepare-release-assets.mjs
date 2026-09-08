import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export function prepareAssets({ version, tag, repository, platform, bundleRoot, outputDir = 'release-assets', notes = '' }) {
  if (!/^\d+\.\d+\.\d+$/.test(version) || tag !== `v${version}` || !/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error('Invalid release identity');
  if (!['windows', 'macos'].includes(platform)) throw new Error('Unsupported release platform');
  const list = root => fs.readdirSync(root, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(root, entry.name);
    return entry.isDirectory() && !entry.name.endsWith('.app') ? list(file) : entry.isFile() ? [file] : [];
  });
  const files = list(bundleRoot);
  fs.mkdirSync(outputDir, { recursive: true });
  const copy = (file, name = path.basename(file).replace(/\s+/g, '.')) => {
    if (fs.existsSync(path.join(outputDir, name))) throw new Error(`Duplicate asset: ${name}`);
    fs.copyFileSync(file, path.join(outputDir, name));
    return name;
  };
  let updater;
  let updaterName;
  let keys;
  if (platform === 'windows') {
    const installers = files.filter(file => file.includes(version) && /\.(exe|msi)$/.test(file));
    const nsis = installers.filter(file => file.endsWith('.exe'));
    if (nsis.length !== 1) throw new Error('Expected exactly one signed NSIS installer');
    updater = nsis[0];
    for (const file of installers) {
      const name = copy(file);
      if (file === updater) updaterName = name;
      if (fs.existsSync(`${file}.sig`)) copy(`${file}.sig`);
    }
    keys = ['windows-x86_64', 'windows-x86_64-nsis'];
  } else {
    const dmgs = files.filter(file => file.includes(version) && file.endsWith('.dmg'));
    const archives = files.filter(file => file.endsWith('.app.tar.gz'));
    if (dmgs.length !== 1 || archives.length !== 1) throw new Error('Expected one universal DMG and one updater archive');
    copy(dmgs[0]);
    updater = archives[0];
    updaterName = path.basename(updater).replace(/\.app\.tar\.gz$/, `_${version}_universal.app.tar.gz`).replace(/\s+/g, '.');
    copy(updater, updaterName);
    copy(`${updater}.sig`, `${updaterName}.sig`);
    keys = ['darwin-aarch64', 'darwin-aarch64-app', 'darwin-x86_64', 'darwin-x86_64-app'];
  }
  const signature = fs.readFileSync(`${updater}.sig`, 'utf8').trim();
  if (!signature) throw new Error('Missing updater signature');
  const url = `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(updaterName)}`;
  const manifest = { version, notes, platforms: Object.fromEntries(keys.map(key => [key, { signature, url }])) };
  fs.writeFileSync(path.join(outputDir, `manifest-${platform}.json`), `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const [, , version, tag, repository, platform, bundleRoot, notesPath = 'release-notes.md'] = process.argv;
  prepareAssets({ version, tag, repository, platform, bundleRoot, notes: fs.readFileSync(notesPath, 'utf8').trim() });
}
