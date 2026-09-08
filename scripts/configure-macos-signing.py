#!/usr/bin/env python3
"""Upload signing secrets directly to GitHub; never print their contents."""
import argparse
import base64
import getpass
import pathlib
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('certificate', type=pathlib.Path, help='Developer ID Application certificate and private key exported as .p12')
parser.add_argument('--repo', action='append', required=True, help='owner/repository; may be repeated')
args = parser.parse_args()
for repo in args.repo:
    if not re.fullmatch(r'[\w.-]+/[\w.-]+', repo):
        parser.error('Invalid repository name')
password = getpass.getpass('Password protecting the exported .p12: ')
result = subprocess.run(['openssl', 'pkcs12', '-in', str(args.certificate), '-clcerts', '-nokeys', '-passin', 'stdin'], input=password+'\n', text=True, capture_output=True)
if result.returncode:
    raise SystemExit('Cannot read the certificate. Check the .p12 password and export format.')
subject = subprocess.run(['openssl', 'x509', '-noout', '-subject', '-nameopt', 'sep_multiline'], input=result.stdout, text=True, capture_output=True, check=True).stdout
match = re.search(r'^\s*CN\s*=\s*(Developer ID Application: .+)$', subject, re.M)
if not match:
    raise SystemExit('This is not a Developer ID Application certificate. App Store distribution certificates cannot be used here.')
identity = match.group(1).strip()
team = re.search(r'\(([A-Z0-9]+)\)$', identity)
if not team:
    raise SystemExit('Could not identify the Apple team from the certificate.')
print('Certificate:', identity)
apple_id = input('Apple ID for notarization: ').strip()
apple_password = getpass.getpass('Apple app-specific password (not your normal account password): ')
if not password or not apple_id or not apple_password:
    raise SystemExit('All values are required; no secrets were uploaded.')
secrets = {
    'APPLE_CERTIFICATE': base64.b64encode(args.certificate.read_bytes()).decode(),
    'APPLE_CERTIFICATE_PASSWORD': password,
    'APPLE_SIGNING_IDENTITY': identity,
    'APPLE_ID': apple_id,
    'APPLE_PASSWORD': apple_password,
    'APPLE_TEAM_ID': team.group(1),
}
for repo in args.repo:
    for name, value in secrets.items():
        subprocess.run(['gh', 'secret', 'set', name, '--repo', repo], input=value, text=True, check=True)
    print('Configured Mac signing and notarization for', repo)
