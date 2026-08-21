# MusicBee tag contract

Verified against the current MusicBee configuration and representative local MP3 files on 2026-08-21. Aurora 0.1.0 does not write these tags.

## Local format

The current export contains 1,096,162 tracks and every track is MP3. MusicBee is configured to store ratings in files, prefer ID3v2.3, retain ID3v1, and enable half-star ratings.

| Aurora value | MusicBee MP3 representation |
| --- | --- |
| Rating | ID3v2 `POPM`, owner `MusicBee`, four-byte counter `0` |
| Loved | `TXXX` description `LOVE RATING`, value `L` |
| Banned | `TXXX` description `LOVE RATING`, value `B` |
| Neutral | Remove only the `LOVE RATING` TXXX |
| Release Year in ID3v2.3 | `TXXX` description `TDRL` |
| Release Year in ID3v2.4 | Native `TDRL`; update a legacy `TXXX/TDRL` too when both exist |

### MusicBee POPM map

| Stars | Byte | Stars | Byte |
| ---: | ---: | ---: | ---: |
| 0.5 | 13 | 3.0 | 128 |
| 1.0 | 1 | 3.5 | 186 |
| 1.5 | 54 | 4.0 | 196 |
| 2.0 | 64 | 4.5 | 242 |
| 2.5 | 118 | 5.0 | 255 |

Unrated means the `MusicBee` POPM frame is absent. A present byte `0` is reserved for MusicBee's explicit zero-star/bomb state and must be fixture-tested before Aurora enables that write.

Local evidence lives at `C:\MusicBeeNew\AppData\MusicBee3Settings.ini` and `C:\MusicBeeNew\Configuration.xml`. The conventions align with the [ID3v2.3 POPM specification](https://id3.org/id3v2.3.0) and MusicBee's published tag mapping.

## Required writer safety

The first writer should target MP3 with the Rust `id3` crate and:

1. Preserve the existing ID3 version; create v2.3 only when no ID3v2 tag exists.
2. Mutate only MusicBee's POPM, `LOVE RATING`, and the appropriate release frame.
3. Preserve artwork, lyrics, MusicBrainz fields, unknown frames, ID3v1, and audio bytes.
4. Write to a same-directory temporary copy, read it back, and verify both target frames and unchanged audio payload.
5. Recheck original size, modification time, and file identity immediately before replacement.
6. Replace with Windows `ReplaceFileW`, retain a backup, and keep a durable pending-write journal until verification succeeds.

MusicBee's continuous folder monitoring is disabled locally. Aurora will need immediate optimistic state and a documented MusicBee rescan path.
