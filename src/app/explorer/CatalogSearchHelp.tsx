import { useEffect, useState } from "react";
import { loadGenreNames } from "../../genres";
import { searchSuggestions, searchSyntaxHint } from "./searchHelp";
import type { ExplorerView } from "../../components/explorer/DeepExplorer";

interface CatalogSearchHelpProps {
  query: string;
  view: ExplorerView;
  onInsert: (query: string) => void;
}

export function CatalogSearchHelp({ query, view, onInsert }: CatalogSearchHelpProps) {
  const [genreNames, setGenreNames] = useState<string[]>([]);
  useEffect(() => {
    let active = true;
    void loadGenreNames().then((names) => {
      if (active) setGenreNames(names);
    }).catch(() => { /* The guide's built-in examples are still available. */ });
    return () => { active = false; };
  }, []);
  const { fields, matches } = searchSuggestions(query, view, genreNames);
  return (
    <section className="search-help" id="catalog-search-help" aria-label="Catalog search syntax">
      <strong>Search syntax</strong>
      <p>Combine filters with AND or a comma. Use OR for alternatives and NOT to exclude. Quotes match a full value. Ranges include both ends.</p>
      <p>Use : or = after a field. Press Enter to search now.</p>
      <CatalogSearchHint query={query} />
      {matches.length > 0 && (
        <div className="search-help__suggestions" aria-label="Completions">
          {matches.map((match) => (
            <button type="button" key={match.label} onClick={() => onInsert(match.query)}>{match.label}</button>
          ))}
        </div>
      )}
      <dl>
        {fields.map(({ name, label, example }) => (
          <div key={name}>
            <dt><code>{name}:</code> {label}</dt>
            <dd><code>{example}</code></dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

export function CatalogSearchHint({ query }: { query: string }) {
  const hint = searchSyntaxHint(query);
  return <span className="search-syntax-hint" id="catalog-search-hint" aria-live="polite">{hint}</span>;
}
