import fs from 'node:fs';
import path from 'node:path';
const env = process.env;
for (const key of ['APPLE_CERTIFICATE', 'APPLE_CERTIFICATE_PASSWORD', 'APPLE_SIGNING_IDENTITY']) {
  if (!env[key]) throw new Error(`Configure repository secret ${key} for notarized Mac releases`);
}
if (!env.APPLE_SIGNING_IDENTITY.startsWith('Developer ID Application:')) throw new Error('Use a Developer ID Application certificate for GitHub distribution');
const api = env.APPLE_API_KEY && env.APPLE_API_ISSUER && env.APPLE_API_PRIVATE_KEY;
const appleId = env.APPLE_ID && env.APPLE_PASSWORD && env.APPLE_TEAM_ID;
if (!api && !appleId) throw new Error('Configure complete Apple API notarization credentials or APPLE_ID, APPLE_PASSWORD (app-specific), and APPLE_TEAM_ID');
fs.writeFileSync(path.join(env.RUNNER_TEMP, 'developer-id.p12'), Buffer.from(env.APPLE_CERTIFICATE, 'base64'), { mode: 0o600 });
if (api) {
  const keyPath = path.join(env.RUNNER_TEMP, 'notarization.p8');
  fs.writeFileSync(keyPath, env.APPLE_API_PRIVATE_KEY, { mode: 0o600 });
  fs.appendFileSync(env.GITHUB_ENV, `APPLE_API_KEY_PATH=${keyPath}\n`);
}
