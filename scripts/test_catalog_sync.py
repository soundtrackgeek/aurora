"""Exercise real SQLite snapshots, orphaned WALs, and replacement failures."""
import contextlib
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import catalog_sync as sync


class CatalogSyncTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / 'pc.sqlite3'
        self.destination = self.root / 'mac.sqlite3'
        self.share = self.root / 'published'
        for path, setting in ((self.source, 'PC'), (self.destination, 'Mac')):
            with contextlib.closing(sqlite3.connect(path)) as db:
                db.executescript('''
                    PRAGMA user_version=67;
                    CREATE TABLE tracks(id);
                    INSERT INTO tracks VALUES(1);
                    CREATE TABLE albums(id);
                    INSERT INTO albums VALUES(1);
                    CREATE TABLE app_settings(id, cover_source_path);
                ''')
                db.execute('INSERT INTO app_settings VALUES(1, ?)', (setting,))
                db.commit()
        sync.publish(self.source, self.share)
        self.closed = patch.object(sync, 'catalog_closed')
        self.closed.start()
        self.addCleanup(self.closed.stop)
        self.apps = patch.object(sync, 'apps_closed')
        self.apps.start()
        self.addCleanup(self.apps.stop)

    def query(self, path, sql):
        with contextlib.closing(sync.connect(path)) as db:
            return db.execute(sql).fetchone()[0]

    def orphan_wal(self):
        # Exit without SQLite cleanup, as with a stopped writer or a final
        # read-only app connection. The schema and settings exist only in WAL.
        subprocess.run([sys.executable, '-c', '''
import os, sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.execute('PRAGMA journal_mode=WAL')
db.execute('PRAGMA wal_autocheckpoint=0')
db.execute('PRAGMA user_version=68')
db.execute("UPDATE app_settings SET cover_source_path='Mac WAL'")
db.execute('INSERT INTO tracks VALUES(2)')
db.commit()
os._exit(0)
''', str(self.destination)], check=True)

    def backups(self):
        return list(self.root.glob('music-library.before-refresh-*.sqlite3'))

    def test_refresh_preserves_settings_and_standalone_rollback(self):
        before = self.destination.read_bytes()
        sync.refresh(self.share, self.destination)
        self.assertEqual(self.query(self.destination, 'SELECT cover_source_path FROM app_settings'), 'Mac')
        self.assertEqual(len(self.backups()), 1)
        self.assertEqual(self.backups()[0].read_bytes(), before)

    def test_orphan_wal_schema_settings_and_backup_are_preserved(self):
        self.orphan_wal()
        with contextlib.closing(sqlite3.connect(self.source)) as db:
            db.execute('PRAGMA user_version=68')
        sync.publish(self.source, self.share)
        self.assertEqual(int.from_bytes(self.destination.read_bytes()[60:64], 'big'), 67)
        sync.refresh(self.share, self.destination)
        self.assertEqual(self.query(self.destination, 'PRAGMA user_version'), 68)
        self.assertEqual(self.query(self.destination, 'SELECT cover_source_path FROM app_settings'), 'Mac WAL')
        self.assertEqual(self.query(self.destination, 'SELECT count(*) FROM tracks'), 1)
        backup = self.backups()[0]
        self.assertFalse(Path(str(backup) + '-wal').exists())
        with contextlib.closing(sqlite3.connect(backup.as_uri() + '?immutable=1', uri=True)) as db:
            self.assertEqual(db.execute('SELECT count(*) FROM tracks').fetchone()[0], 2)
            self.assertEqual(db.execute('SELECT cover_source_path FROM app_settings').fetchone()[0], 'Mac WAL')

    def test_checksum_failure_preserves_catalog(self):
        manifest = json.loads((self.share / 'catalog-latest.json').read_text())
        with (self.share / manifest['file']).open('ab') as stream:
            stream.write(b'corrupt')
        before = self.destination.read_bytes()
        with self.assertRaisesRegex(RuntimeError, 'checksum'):
            sync.refresh(self.share, self.destination)
        self.assertEqual(self.destination.read_bytes(), before)
        self.assertEqual(self.backups(), [])
        self.assertEqual(list(self.root.glob('.catalog-refresh-*')), [])

    def test_schema_mismatch_is_still_rejected(self):
        self.orphan_wal()
        with self.assertRaisesRegex(RuntimeError, 'schema differs'):
            sync.refresh(self.share, self.destination)
        self.assertEqual(self.query(self.destination, 'SELECT count(*) FROM tracks'), 2)
        self.assertEqual(self.backups(), [])

    def test_invalid_wal_is_never_deleted(self):
        wal = Path(str(self.destination) + '-wal')
        wal.write_bytes(b'pending')
        before = self.destination.read_bytes()
        with self.assertRaisesRegex(RuntimeError, 'WAL is incomplete or unrecognized'):
            sync.refresh(self.share, self.destination)
        self.assertEqual(wal.read_bytes(), b'pending')
        self.assertEqual(self.destination.read_bytes(), before)

    def test_rollback_journal_is_never_deleted(self):
        journal = Path(str(self.destination) + '-journal')
        journal.write_bytes(b'pending')
        with self.assertRaisesRegex(RuntimeError, 'rollback journal'):
            sync.refresh(self.share, self.destination)
        self.assertEqual(journal.read_bytes(), b'pending')

    def test_busy_checkpoint_preserves_committed_data(self):
        self.orphan_wal()
        with contextlib.closing(sqlite3.connect(self.destination)) as writer:
            writer.execute('BEGIN IMMEDIATE')
            with self.assertRaisesRegex(RuntimeError, 'checkpoint is busy'):
                sync.checkpoint_catalog(self.destination, timeout=0.01)
            self.assertGreater(Path(str(self.destination) + '-wal').stat().st_size, 0)
            self.assertEqual(writer.execute('SELECT count(*) FROM tracks').fetchone()[0], 2)
            writer.rollback()
        self.assertEqual(self.backups(), [])

    def test_main_file_change_during_download_aborts_replacement(self):
        copy = sync.shutil.copyfile

        def changed(source, stage):
            result = copy(source, stage)
            with contextlib.closing(sqlite3.connect(self.destination)) as db:
                db.execute("UPDATE app_settings SET cover_source_path='Changed'")
                db.commit()
            return result

        with patch.object(sync.shutil, 'copyfile', side_effect=changed):
            with self.assertRaisesRegex(RuntimeError, 'changed during download'):
                sync.refresh(self.share, self.destination)
        self.assertEqual(self.query(self.destination, 'SELECT cover_source_path FROM app_settings'), 'Changed')
        self.assertEqual(self.backups(), [])

    def test_app_reopened_during_download_aborts_replacement(self):
        sync.catalog_closed.side_effect = [None, RuntimeError('Mac catalog is open')]
        before = self.destination.read_bytes()
        with self.assertRaisesRegex(RuntimeError, 'catalog is open'):
            sync.refresh(self.share, self.destination)
        self.assertEqual(self.destination.read_bytes(), before)
        self.assertEqual(self.backups(), [])

    def test_wal_created_during_download_aborts_replacement(self):
        copy = sync.shutil.copyfile

        def changed(source, stage):
            result = copy(source, stage)
            self.orphan_wal()
            return result

        with patch.object(sync.shutil, 'copyfile', side_effect=changed):
            with self.assertRaisesRegex(RuntimeError, 'became active'):
                sync.refresh(self.share, self.destination)
        self.assertEqual(self.query(self.destination, 'SELECT cover_source_path FROM app_settings'), 'Mac WAL')
        self.assertEqual(self.backups(), [])


class CatalogUseTests(unittest.TestCase):
    def test_open_handles_block_refresh_even_without_sqlite_locks(self):
        result = subprocess.CompletedProcess([], 0, 'p123\n', '')
        with patch.object(sync, 'apps_closed'), patch.object(sync.subprocess, 'run', return_value=result):
            with self.assertRaisesRegex(RuntimeError, 'open in another process'):
                sync.catalog_closed(Path('/tmp/catalog.sqlite3'))

    def test_failed_open_handle_check_blocks_refresh(self):
        result = subprocess.CompletedProcess([], 2, '', 'lsof failed')
        with patch.object(sync, 'apps_closed'), patch.object(sync.subprocess, 'run', return_value=result):
            with self.assertRaisesRegex(RuntimeError, 'Could not check'):
                sync.catalog_closed(Path('/tmp/catalog.sqlite3'))

    def test_no_open_handles_allows_refresh(self):
        result = subprocess.CompletedProcess([], 1, '', '')
        with patch.object(sync, 'apps_closed'), patch.object(sync.subprocess, 'run', return_value=result):
            sync.catalog_closed(Path('/tmp/catalog.sqlite3'))

    def test_named_app_blocks_refresh(self):
        for executable in ('aurora', 'music-library'):
            with self.subTest(executable=executable):
                with patch.object(sync.subprocess, 'check_output', return_value='/Applications/App.app/Contents/MacOS/' + executable):
                    with self.assertRaisesRegex(RuntimeError, 'Quit Aurora and Music Library'):
                        sync.apps_closed()


if __name__ == '__main__':
    unittest.main()
