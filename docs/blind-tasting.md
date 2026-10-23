# Blind tasting

Opportunity 10 is implemented in Aurora 0.46.0 and Tonehavn 0.53.0 / iOS build 70 (2026-10-23).
Music Library remains the owner of the analysis and Aurora's saved catalog playlists.
Tonehavn uses its existing versioned sonic publication and owns its independent playlists and downloads.

## Listen first

In Aurora, open a track inspector and choose **More like this → Blind tasting**.
In Tonehavn web, open a song's **Explore this sound** actions and choose **Blind tasting**.
On iPhone, use **Blind tasting** in Explore, Downloads, or a song's sound actions.

Prepare a round, play the seed and the two anonymous A/B candidates, then choose
**Keep A/B playing · reveal**. The chosen sound continues (or starts from the beginning
if the other sound was playing). There is no correct answer, score, rating prediction,
automatic Love or Ban. You can also reveal without choosing. The reveal shows the title,
artist, album, artwork and whole-track acoustic distance. **Save** uses the existing
Music Library playlist dialog in Aurora and creates a regular, single-track Tonehavn
playlist in Tonehavn. **Hear the album** plays the full available catalog album.
**Another round** excludes the last 300 offered track IDs/keys for that session.

Playback uses complete songs and manual seeking, never an allegedly ideal excerpt.
Tasting takes over playback; closing stops its audition. It does not restore the prior
queue. Normal listening history may record auditions. A preference itself is only UI
state and changes no rating, love/ban flag, audio tags or source files.

## Concealment and levels

The modal/full-screen screen and its accessibility tree hide candidate titles,
artists, ratings and artwork until reveal. Local media-control metadata uses anonymous
labels throughout the round. This is a listening activity, not a security boundary:
audio may be recognizable, developer tools contain the reveal data, and other devices,
external integrations or the shared Tonehavn server queue can expose identity.

- Aurora applies a temporary Track ReplayGain policy, using existing measured tags
  or its existing calibrated sonic loudness estimate (with its limiter for estimated
  boosts). Missing normalization fails the audition. Estimates reduce level bias but
  are not measured LUFS. Saved audio settings do not change.
- Tonehavn web explicitly uses track-normalized Very High AAC during tasting. Its
  saved quality/normalization settings do not change. The usual playback lease and
  queue revision guards still apply. The recording's source token is checked before
  audio starts; a changed source requires a new round.
- iPhone uses complete downloaded originals with measured R128 track gains and true
  peaks. A common target is lowered for all three sounds until none needs a boost and
  every measured true peak stays at or below -1 dBTP. Playback cannot start with absent,
  non-finite or mismatched loudness/file tokens. Original files remain unchanged.

## A round on the train

On iPhone, prepare a round while connected and press **Download round / refresh levels**.
The three downloads use the existing download manager; all must finish and acquire
matching measured loudness before Play becomes available. The most recently prepared
round is saved on this iPhone, separately for each server origin. Reopen it through
**Explore → Blind tasting** or **Downloads → Blind tasting**, including offline.
Reveal and the local choice work offline. Reconnect for another round, a playlist save
or full-album listening. This version retains one round, not an automatically refilling
pack. Deleting a required download disables playback until the round is complete again.

## Candidate policy

Both adapters use Music Library's existing current-profile whole-track analysis and
exact weighted index, a bounded neighborhood of 4,096 candidates and a maximum sound
distance of 0.25. This cutoff is a product heuristic, not a perceptual guarantee.
The seed and candidates must have available unchanged media and usable published/current
analysis. Existing ban eligibility applies. Exclude the seed recording, the seed album,
identical audio payload hashes, duplicate normalized title-and-performer credits and
repeated candidate albums. Missing artist/album metadata does not prove identity.
No rule is relaxed to fill the pair; fewer than two eligible recordings produces an
explanation. A/B order is randomized independently of acoustic rank.

Aurora reuses the existing read-only worlds adapter with no metadata boundary.
Tonehavn adds the authenticated, CSRF-protected `/api/v1/sonic/tasting` contract,
`blind-tasting-v1`, over its immutable published sonic catalog. It returns two candidates,
source tokens, catalog revision and reveal evidence, with no raw vectors or filesystem
paths. No new audio analysis pass or external AI service is needed.

## Verification and limits

Regression tests cover full-pair requirements, freshness, identical audio/song credits,
excluded rounds, absent analysis, concealed UI, explicit saves, stale async responses,
and source-token/loudness guards. Swift tests cover equal measured output levels,
incomplete downloads, anonymous media metadata and cleanup; Xcode/iPhone execution is
still required on a Mac/device. Browser checks use sample data and establish interaction
and layout, not native audio quality. Device listening, perceived matching quality,
lock-screen presentation and background/offline behavior need the acceptance run.
