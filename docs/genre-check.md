# Genre check by sound

**Genres → Check genres by sound** (0.68.0) listens to the whole library and lists albums whose genre tag does not match their sound. They open in **Review genre changes**, the same review as genre proposals from AI assistants. Nothing is written until you tick albums and choose **Write genre**.

It needs Music Library's audio analysis (CLAP sound search, Music Library 0.216.0 or newer) and Aurora's sound descriptor cache, which Aurora builds a few minutes after it starts. A check of about 73,000 albums takes 10 to 20 seconds.

## What it finds

**Tag errors.** For each genre, the average of its albums' sound is that genre's sound. An album is flagged when all of these are true:

- It sounds clearly more like another genre's average than its own. Its own average leaves the album out, so it cannot vouch for itself.
- It is among the 5% least typical albums of its own genre.
- It sounds at least as typical of the other genre as that genre's median album.

Each album is at least 0.25 closer to the other genre in cosine similarity. That is **medium confidence**: mostly judgment calls between neighbouring genres. At 0.4 or more it is **high confidence**: on the real library these were mostly plain mistakes, such as a groove metal album tagged Rock or a hip hop album tagged Jazz.

**More specific genres.** An album tagged with a broad genre such as Rock or Pop gets the specific genre that most of its 15 nearest-sounding albums carry. Only albums with a specific genre take part in that vote. It needs 9 of 15 votes (medium confidence) and 12 of 15 (high confidence). For example, a Rock album whose nearest albums are nearly all Hard Rock gets Hard Rock.

Tag errors come first, then refinements, each strongest first, up to 500 albums. After you write or dismiss them, run the check again for the next ones.

## What it suggests

Only genres you already use on at least 30 albums, so suggestions stay within your own genre names (Rate Your Music genres for this library). Never suggested:

- **Not a sound.** Score genres named after the kind of film or game (Action, Animation, Anime, Comedy, Documentary, Drama, Fantasy, Horror, Sci-Fi, Thriller, TV, Video Game, Western) and Soundtrack, Trailer, Christmas Music and Children's Music. Albums with these genres are never checked either.
- **Broad.** RYM's top-level genres (Rock, Pop, Electronic, Metal, Jazz, Country, Blues, Folk, Classical, Hip Hop, Rap, Dance, Punk, R&B, Experimental) and catch-all names that are not RYM genres (Electronica, Oldies, Funk / Soul, Classic, Vocal, Visa, Instrumental). Albums with these genres are refined.
- **Other spellings.** When two genres differ only in case, punctuation or `&` versus "and", such as Rock & Roll and Rock And Roll, the less common spelling is never suggested, and its albums are refined towards the common one.

The genre field in the review stays editable.

## Evidence

Each album shows:

- Its confidence.
- Why it was flagged, with the numbers. For example: "Sounds more like Hard Rock than Synthwave: closer to Hard Rock's average sound than 72% of Hard Rock albums, and further from Synthwave's than 98% of Synthwave albums."
- Up to three songs of the suggested genre that sound most like it. At most one song comes from each album; they are taken from the five albums of that genre closest in sound.

Medium-confidence albums start unticked.

## Limits

- The check hears the sound, not the context. A pop album by a metal band, a live album or a compilation can sound unlike its genre and still be tagged correctly.
- Genres that sound alike, such as Hard Rock, AOR and Glam Metal, are often close calls; most of these are medium confidence.
- Albums without analyzed tracks are not checked.

The check is in `src-tauri/src/genre_check.rs`; its read-only test `real_library_proof` prints a sample of findings for the real library.
