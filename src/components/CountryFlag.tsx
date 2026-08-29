import { useEffect, useState } from "react";
import "./CountryFlag.css";

interface CountryFlagProps {
  code?: string | null;
  name?: string | null;
  className?: string;
  ariaLabel?: string;
}

function flagCode(value?: string | null): string | null {
  const normalized = value?.trim().toLocaleLowerCase() ?? "";
  const canonical = normalized === "uk" ? "gb" : normalized;
  return /^[a-z]{2}$/u.test(canonical) ? canonical : null;
}

const flagUrls = import.meta.glob("/node_modules/flag-icons/flags/4x3/*.svg", {
  import: "default",
  query: "?url",
}) as Record<string, () => Promise<string>>;

const loadedFlags = new Map<string, string>();
const pendingFlags = new Map<string, Promise<string>>();

function loadFlag(code: string): Promise<string> | null {
  const loader = flagUrls[`/node_modules/flag-icons/flags/4x3/${code}.svg`];
  if (!loader) return null;
  const cached = loadedFlags.get(code);
  if (cached) return Promise.resolve(cached);
  const pending = pendingFlags.get(code) ?? loader();
  pendingFlags.set(code, pending);
  return pending.then((url) => {
    loadedFlags.set(code, url);
    pendingFlags.delete(code);
    return url;
  }, (error: unknown) => {
    pendingFlags.delete(code);
    throw error;
  });
}

export function CountryFlag({ code, name, className = "", ariaLabel }: CountryFlagProps) {
  const normalizedCode = flagCode(code);
  if (!normalizedCode) return null;

  const countryName = name?.trim() || code?.trim().toLocaleUpperCase() || "Unknown country";
  return <LoadedCountryFlag code={normalizedCode} countryName={countryName} className={className} ariaLabel={ariaLabel} />;
}

function LoadedCountryFlag({ code, countryName, className, ariaLabel }: { code: string; countryName: string; className: string; ariaLabel?: string }) {
  const [loaded, setLoaded] = useState<{ code: string; url: string } | null>(() => (
    loadedFlags.has(code) ? { code, url: loadedFlags.get(code) ?? "" } : null
  ));
  const url = loaded?.code === code ? loaded.url : null;

  useEffect(() => {
    const request = loadFlag(code);
    if (!request) return undefined;
    let active = true;
    void request.then((nextUrl) => {
      if (active) setLoaded({ code, url: nextUrl });
    }).catch(() => undefined);
    return () => {
      active = false;
    };
  }, [code]);

  return (
    <span
      className={`aurora-country-flag${url ? " is-loaded" : ""}${className ? ` ${className}` : ""}`}
      role="img"
      aria-label={ariaLabel ?? `${countryName} origin country`}
      title={countryName}
    >
      {url ? <img src={url} alt="" aria-hidden="true" /> : null}
    </span>
  );
}
