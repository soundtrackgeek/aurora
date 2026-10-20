#!/usr/bin/env python3
"""Publish a SQLite catalog on Windows; install its verified snapshot on Mac."""
import argparse, contextlib, hashlib, json, os, re, shutil, sqlite3, subprocess, sys, tempfile, time
from urllib.parse import unquote, urlsplit
from pathlib import Path


def connect(path):
    return sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True, timeout=10)


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(4 * 1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def verify(path):
    with contextlib.closing(connect(path)) as c:
        if c.execute('PRAGMA quick_check').fetchall() != [('ok',)]:
            raise RuntimeError('SQLite integrity check failed')
        version = c.execute('PRAGMA user_version').fetchone()[0]
        tracks = c.execute('SELECT count(*) FROM tracks').fetchone()[0]
        albums = c.execute('SELECT count(*) FROM albums').fetchone()[0]
        if not tracks or not albums:
            raise RuntimeError('Refusing an empty catalog')
        return dict(schema=version, tracks=tracks, albums=albums)


def apps_closed():
    ps = subprocess.check_output(['ps', '-axo', 'comm='], text=True)
    if any(line.strip().endswith(('/aurora', '/music-library')) for line in ps.splitlines()):
        raise RuntimeError('Quit Aurora and Music Library before refreshing, then try again.')


def catalog_closed(destination):
    apps_closed()
    # SQLite permits idle connections that hold no locks. Check open handles too,
    # including unknown readers, before checkpointing or replacing the catalog.
    paths = [str(destination) + suffix for suffix in ('', '-wal', '-shm', '-journal')]
    result = subprocess.run(['/usr/sbin/lsof', '-F', 'p', '--', *paths],
                            capture_output=True, text=True)
    if result.returncode not in (0, 1):
        raise RuntimeError('Could not check whether the Mac catalog is open: ' + result.stderr.strip())
    if result.returncode == 0 or result.stdout.strip():
        raise RuntimeError('Mac catalog is open in another process. Quit all catalog users before refreshing.')


def require_no_pending_changes(destination):
    for suffix in ('-wal', '-journal'):
        sidecar = Path(str(destination) + suffix)
        if sidecar.exists() and sidecar.stat().st_size:
            raise RuntimeError('Catalog became active or still has pending SQLite changes; retry with apps closed')


def checkpoint_catalog(destination, timeout=10):
    catalog_closed(destination)
    journal = Path(str(destination) + '-journal')
    if journal.exists() and journal.stat().st_size:
        raise RuntimeError('Mac catalog has a rollback journal. Open and cleanly quit Music Library first; do not delete it.')
    wal = Path(str(destination) + '-wal')
    if wal.exists() and wal.stat().st_size:
        # SQLite can silently ignore an invalid WAL header. Do not discard an
        # unreadable log when making the standalone rollback copy below.
        with wal.open('rb') as stream:
            header = stream.read(32)
        page_size = int.from_bytes(header[8:12], 'big')
        if (len(header) != 32 or header[:4] not in (b'\x37\x7f\x06\x82', b'\x37\x7f\x06\x83')
                or int.from_bytes(header[4:8], 'big') != 3007000
                or page_size < 512 or page_size > 65536 or page_size & (page_size - 1)
                or (wal.stat().st_size - 32) % (page_size + 24)):
            raise RuntimeError('Mac catalog WAL is incomplete or unrecognized. Keep it and recover with Music Library first.')
        print('Checkpointing committed Mac catalog changes...', flush=True)
        try:
            with contextlib.closing(sqlite3.connect(destination.resolve().as_uri() + '?mode=rw',
                                                   uri=True, timeout=timeout)) as db:
                result = db.execute('PRAGMA wal_checkpoint(TRUNCATE)').fetchone()
                if result != (0, 0, 0):
                    raise RuntimeError('Mac catalog checkpoint is busy. Quit all catalog users and retry; existing catalog is preserved.')
        except sqlite3.Error as error:
            raise RuntimeError(f'Could not checkpoint Mac catalog; existing catalog and SQLite logs are preserved: {error}') from error
    require_no_pending_changes(destination)


def catalog_identity(destination):
    stat = destination.stat()
    return stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns


def publish(source, folder):
    if not source.is_file():
        raise RuntimeError(f'Catalog not found: {source}')
    folder.mkdir(parents=True, exist_ok=True)
    # SQLite backup runs on the PC, where the live database and its WAL reside.
    # Never open the live PC database through SMB from the Mac.
    with tempfile.TemporaryDirectory(prefix='catalog-publish-') as work:
        snapshot = Path(work) / 'catalog.sqlite3'
        print('Creating consistent local snapshot...', flush=True)
        deadline = time.monotonic() + 600
        def progress(*_):
            if time.monotonic() > deadline:
                raise RuntimeError('Backup timed out; previous publication is untouched')
        with contextlib.closing(connect(source)) as src, contextlib.closing(sqlite3.connect(snapshot)) as dst:
            src.backup(dst, pages=2048, progress=progress, sleep=0.1)
            dst.execute('PRAGMA journal_mode=DELETE')
        info = verify(snapshot)
        sha = digest(snapshot)
        name = f'music-library-{sha}.sqlite3'
        target = folder / name
        if not target.exists():
            fd, temporary = tempfile.mkstemp(prefix='.catalog-upload-', dir=folder)
            os.close(fd)
            try:
                shutil.copyfile(snapshot, temporary)
                with open(temporary, 'rb+') as f:
                    f.flush(); os.fsync(f.fileno())
                if digest(Path(temporary)) != sha:
                    raise RuntimeError('Uploaded snapshot checksum differs')
                os.replace(temporary, target)
            finally:
                Path(temporary).unlink(missing_ok=True)
        elif digest(target) != sha:
            raise RuntimeError('Existing published snapshot has an invalid checksum')
        manifest = dict(file=name, sha256=sha, bytes=snapshot.stat().st_size,
                        published_at=time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), **info)
        fd, temporary = tempfile.mkstemp(prefix='.manifest-', dir=folder)
        try:
            with os.fdopen(fd, 'w') as f:
                json.dump(manifest, f, indent=2); f.flush(); os.fsync(f.fileno())
            os.replace(temporary, folder / 'catalog-latest.json')
        finally:
            Path(temporary).unlink(missing_ok=True)
        print('Published:', json.dumps(manifest), flush=True)


def refresh(folder, destination):
    apps_closed()
    manifest = json.loads((folder / 'catalog-latest.json').read_text())
    name = manifest['file']
    if Path(name).name != name or not name.startswith('music-library-') or not name.endswith('.sqlite3'):
        raise RuntimeError('Invalid snapshot filename')
    source = folder / name
    if not destination.is_file():
        raise RuntimeError('Existing Mac catalog is required to preserve local settings')
    checkpoint_catalog(destination)
    original_identity = catalog_identity(destination)
    old_info = verify(destination)
    if manifest['schema'] != old_info['schema']:
        raise RuntimeError('Catalog schema differs. Update Music Library before refreshing.')
    if shutil.disk_usage(destination.parent).free < manifest['bytes'] + 512 * 1024 * 1024:
        raise RuntimeError('Insufficient free space for a staged catalog')
    with contextlib.closing(connect(destination)) as old:
        old.row_factory = sqlite3.Row
        settings = dict(old.execute('SELECT * FROM app_settings WHERE id=1').fetchone())
    fd, temporary = tempfile.mkstemp(prefix='.catalog-refresh-', suffix='.sqlite3', dir=destination.parent)
    os.close(fd)
    stage = Path(temporary)
    try:
        print('Downloading catalog; this may take several minutes...', flush=True)
        shutil.copyfile(source, stage)
        if stage.stat().st_size != manifest['bytes'] or digest(stage) != manifest['sha256']:
            raise RuntimeError('Snapshot checksum failed; existing catalog is untouched')
        info = verify(stage)
        if any(info[k] != manifest[k] for k in ('schema', 'tracks', 'albums')):
            raise RuntimeError('Snapshot metadata differs from its manifest')
        with contextlib.closing(sqlite3.connect(stage)) as db:
            db.execute('PRAGMA journal_mode=DELETE')
            columns = {r[1] for r in db.execute('PRAGMA table_info(app_settings)')}
            for key, value in settings.items():
                if key in columns and key != 'id':
                    db.execute('UPDATE app_settings SET "' + key.replace('"', '""') + '"=? WHERE id=1', (value,))
            db.commit()
        verify(stage)
        with stage.open('rb+') as f:
            f.flush(); os.fsync(f.fileno())
        catalog_closed(destination)
        require_no_pending_changes(destination)
        if catalog_identity(destination) != original_identity:
            raise RuntimeError('Mac catalog changed during download; retry with apps closed. Existing catalog is preserved.')
        backup = destination.with_name('music-library.before-refresh-' + time.strftime('%Y%m%d-%H%M%S') + '.sqlite3')
        # A hard link retains the complete old file without another multi-GB copy.
        os.link(destination, backup)
        for suffix in ('-wal', '-shm', '-journal'):
            Path(str(destination) + suffix).unlink(missing_ok=True)
        os.replace(stage, destination)
        print(f'Refreshed: {info["tracks"]:,} tracks / {info["albums"]:,} albums.\nRollback copy: {backup}', flush=True)
    finally:
        stage.unlink(missing_ok=True)


def mounted_share_folder(server, share, relative, mount_output=None):
    if relative.is_absolute() or '..' in relative.parts:
        raise RuntimeError('Snapshot path must be relative to the SMB share')
    if mount_output is None:
        mount_output = subprocess.check_output(['/sbin/mount', '-t', 'smbfs'], text=True)
    matches = []
    for line in mount_output.splitlines():
        match = re.match(r'^(//.+) on (.+) \(smbfs(?:,|\))', line)
        if not match:
            continue
        remote = urlsplit('smb:' + match.group(1))
        if (remote.hostname or '').lower() == server.lower() and unquote(remote.path).strip('/').lower() == share.lower():
            matches.append(Path(match.group(2)) / relative)
    if len(matches) != 1:
        raise RuntimeError(f'Connect to smb://{server}/{share} in Finder, then retry. Expected one mounted share; found {len(matches)}.')
    return matches[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['publish', 'refresh'])
    parser.add_argument('--folder', type=Path, required=True)
    parser.add_argument('--smb-server')
    parser.add_argument('--smb-share')
    parser.add_argument('--source', type=Path)
    parser.add_argument('--destination', type=Path, default=Path.home() / 'Library/Application Support/com.local.musiclibrary/music-library.sqlite3')
    args = parser.parse_args()
    if args.smb_server or args.smb_share:
        if args.action != 'refresh' or not args.smb_server or not args.smb_share:
            parser.error('SMB discovery requires refresh, --smb-server and --smb-share')
        args.folder = mounted_share_folder(args.smb_server, args.smb_share, args.folder)
        print(f'Using mounted snapshot folder: {args.folder}', flush=True)
    if args.action == 'publish':
        source = args.source or Path(os.environ['APPDATA']) / 'com.local.musiclibrary/music-library.sqlite3'
        publish(source, args.folder)
    else:
        if sys.platform != 'darwin':
            raise RuntimeError('Run refresh on the Mac')
        refresh(args.folder, args.destination)

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'ERROR: {error}', file=sys.stderr)
        sys.exit(1)
