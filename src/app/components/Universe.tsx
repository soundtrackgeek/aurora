import { formatCount, type Artist } from "../../library";

export function Universe({ artists, activeArtist, onSelect }: { artists: Artist[]; activeArtist: string | null; onSelect: (artist: Artist) => void; }) {
  const visibleArtists = artists.slice(0, 8);
  return (
    <section className="universe" aria-labelledby="universe-title">
      <div className="universe__heading">
        <p className="eyebrow">Your listening gravity</p>
        <h1 id="universe-title">Welcome back, Jørn.</h1>
        <p>Choose a world to focus the library.</p>
      </div>
      <div className="universe__quote">
        <p>“Music is the shorthand of emotion.”</p>
        <span>— Leo Tolstoy</span>
      </div>
      <div className="solar-system" aria-label="Top artists in your music universe">
        <span className="sun" aria-hidden="true" />
        {[0, 1, 2, 3].map((orbit) => <span className={`orbit orbit--${orbit + 1}`} key={orbit} aria-hidden="true" />)}
        {visibleArtists.map((artist, index) => (
          <button
            type="button"
            className={`planet planet--${index + 1}${activeArtist === artist.name ? " is-active" : ""}`}
            key={artist.id}
            onClick={() => onSelect(artist)}
            aria-label={`Explore ${artist.name}, ${formatCount(artist.trackCount)} tracks`}
            aria-pressed={activeArtist === artist.name}
          >
            <span className="planet__dot" />
            <span className="planet__label">{artist.name}</span>
          </button>
        ))}
      </div>
    </section>
  );
}
