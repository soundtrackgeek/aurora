# Publisher logo sourcing

Aurora 0.15.16 reserves a logo slot on the Publishers page but intentionally renders the Aurora publisher fallback unless a verified image is available. Publisher names in Music Library are free text, and neither the catalog nor MusicBrainz guarantees that a label has a reusable logo.

## Recommended source chain

1. Normalize the catalog publisher for matching while retaining the most common stored casing for display.
2. Resolve the publisher to a [MusicBrainz Label](https://musicbrainz.org/doc/Label). MusicBrainz exposes label identity through its [web service](https://musicbrainz.org/doc/MusicBrainz_API) and documents a specific [label-logo relationship](https://musicbrainz.org/relationship/b35f7822-bf3c-4148-b306-fb723c63ee8b).
3. Follow a verified Wikidata relationship and read [logo image property P154](https://www.wikidata.org/wiki/Property:P154). Use the documented [Wikidata data-access routes](https://www.wikidata.org/wiki/Help:Data_access), not HTML scraping.
4. Resolve the Commons file through the [MediaWiki Imageinfo API](https://www.mediawiki.org/wiki/API:Imageinfo) and retain its canonical URL, license, author, attribution text, and source page.
5. Cache only bounded thumbnails and metadata. Respect the [Wikimedia API usage guidelines](https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_API_Usage_Guidelines), and show the attribution required by each file under the [Commons reuse guidance](https://commons.wikimedia.org/wiki/Commons:Reusing_content_outside_Wikimedia/en).

MusicBrainz core data is CC0 while supplementary data has separate terms; an image linked from MusicBrainz or Wikidata does not inherit those database terms. Aurora must evaluate the license of each Commons file independently. The [MusicBrainz database licensing documentation](https://musicbrainz.org/doc/MusicBrainz_Database) and [API rate limits](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting) remain part of the implementation contract.

## Product decision

- Ship no third-party publisher logos in the Aurora binary.
- Keep a deterministic Aurora fallback icon for every publisher.
- Allow future user-provided local overrides without network access.
- Treat MusicBrainz → Wikidata → Commons as optional runtime enrichment with a local cache, explicit provenance, and a license allowlist.
- Do not use Discogs as a general logo source. Its [API terms](https://support.discogs.com/hc/en-us/articles/360009334593-API-Terms-of-Use) restrict content use and caching in ways that do not fit a durable local logo library.

This design keeps the page complete offline and avoids implying that a wordmark is safe to redistribute merely because it appears in a public database.
