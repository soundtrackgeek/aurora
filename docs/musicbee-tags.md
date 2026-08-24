# MusicBee tag contract

Verified against the current MusicBee configuration and representative local MP3 files. Aurora 0.17.0 preserves the existing ID3v2.3/v2.4 version and edits only explicitly selected field families.

## Editable MP3 mapping

| Aurora field | ID3 representation |
| --- | --- |
| Album Artist | `TPE2` |
| Artist | `TXXX`, description `DISPLAY ARTIST`; when absent, Aurora displays the `TPE1` credits joined with `; ` |
| Album | `TALB` |
| Track Title | `TIT2` |
| Genre | `TCON` |
| Publisher | `TPUB` |
| Track Rating | `POPM`, owner `MusicBee`, four-byte counter `0` |
| Year in ID3v2.3 | `TYER` |
| Year in ID3v2.4 | `TDRC` |
| Release Year in ID3v2.3 | `TXXX`, description `TDRL` |
| Release Year in ID3v2.4 | Native `TDRL`; update a legacy `TXXX/TDRL` too when it already exists |
| Track number / total | `TRCK` as `number` or `number/total` |
| Disc number / total | `TPOS` as `number` or `number/total` |
| Loved | Existing inline controls use `TXXX`, description `LOVE RATING`, value `L` |
| Banned | Existing inline controls use `TXXX`, description `LOVE RATING`, value `B` |
| Neutral | Existing inline controls remove only `TXXX/LOVE RATING` |

Blank selected values remove only the corresponding target frame, except Album Artist, Album, and Track Title, which are required so Music Library can safely retain catalog identity. An unselected field is not rewritten, even when the album selection displays it as Mixed. Editing Artist changes only MusicBee's `DISPLAY ARTIST`; the underlying `TPE1` performer-credit values remain byte-for-byte outside the operation.

### MusicBee POPM map

| Stars | Byte | Stars | Byte |
| ---: | ---: | ---: | ---: |
| 0.5 | 13 | 3.0 | 128 |
| 1.0 | 1 | 3.5 | 186 |
| 1.5 | 54 | 4.0 | 196 |
| 2.0 | 64 | 4.5 | 242 |
| 2.5 | 118 | 5.0 | 255 |

Unrated means Aurora removes only the `MusicBee` POPM frame. A present MusicBee POPM byte `0` is also read as unrated; Aurora does not emit byte `0`. POPM frames owned by other applications are preserved.

Local MusicBee configuration evidence lives at `C:\MusicBeeNew\AppData\MusicBee3Settings.ini` and `C:\MusicBeeNew\Configuration.xml`. The conventions align with the [ID3v2.3 POPM specification](https://id3.org/id3v2.3.0) and MusicBee's published tag mapping.

## Writer safety

Aurora's Rust writer:

1. Preserves the existing ID3 version and creates ID3v2.3 only when no ID3v2 tag exists.
2. Preflights every selected MP3 and applies only checked field families.
3. Preserves artwork, lyrics, ReplayGain, MusicBrainz fields, unknown frames, other POPM owners, ID3v1/trailing data, and the audio payload.
4. Writes to a same-directory temporary copy and verifies the full resulting editable value set, unselected frames, and unchanged audio bytes.
5. Rechecks original size, timestamps, and Windows file identity immediately before atomic replacement.
6. Retains a journaled original for rollback, crash recovery, and guarded one-step undo.

After a verified save, Music Library performs the guarded existing-folder import and Aurora's normal revision watcher refreshes the catalog. Aurora never writes the shared Music Library SQLite database itself.
