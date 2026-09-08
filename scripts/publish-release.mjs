import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
const [, , version, tag, product] = process.argv;
if (!/^\d+\.\d+\.\d+$/.test(version) || tag !== `v${version}` || !process.env.GITHUB_SHA) throw new Error('Invalid release identity');
const existing = spawnSync('gh', ['release', 'view', tag, '--json', 'isDraft,targetCommitish'], { encoding: 'utf8' });
if (existing.status === 0) {
  const release = JSON.parse(existing.stdout);
  if (!release.isDraft || release.targetCommitish !== process.env.GITHUB_SHA) throw new Error('Release already published or belongs to another commit; bump the version');
} else {
  execFileSync('gh', ['release', 'create', tag, '--draft', '--target', process.env.GITHUB_SHA, '--title', `${product} ${version}`, '--notes-file', 'release-notes.md'], { stdio: 'inherit' });
}
const files = fs.readdirSync('release-assets').map(file => path.join('release-assets', file));
if (!files.some(file => file.endsWith('latest.json'))) throw new Error('Combined updater manifest is missing');
execFileSync('gh', ['release', 'upload', tag, ...files, '--clobber'], { stdio: 'inherit' });
execFileSync('gh', ['release', 'edit', tag, '--draft=false', '--latest'], { stdio: 'inherit' });
