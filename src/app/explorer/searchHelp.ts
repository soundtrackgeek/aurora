import countries from "flag-icons/country.json";
import fieldsText from "../../searchFields.txt?raw";
import valuesText from "../../searchValues.txt?raw";
import { matchGenreSuggestions } from "../../genreMatch";
import type { ExplorerView } from "../../components/explorer/DeepExplorer";

const fields = fieldsText.trim().split(/\r?\n/).map((line) => {
  const [name, label, example, views = "sat"] = line.split("|");
  return { name, label, example, views };
});
// Field availability codes cover Songs, Albums, Artists, and Aurora NG (n).
const viewCode = { tracks: "s", albums: "a", artists: "t" } as const;
const values = new Map(valuesText.trim().split(/\r?\n/).map((line) => {
  const [name, ...options] = line.split("|");
  return [name, options];
}));
const fieldNames = new Set([...fields.map(({ name }) => name), "singles"]);
const tokenPattern = /"[^"]*(?:"|$)|,|\s+|[^\s,"]+/gu;

export function searchSyntaxHint(query: string): string | null {
  if ((query.match(/"/gu)?.length ?? 0) % 2) return "Close the quoted value with a double quote.";
  let hasClause = false;
  let waitingForValue = false;
  let operator = "";
  for (const match of query.matchAll(tokenPattern)) {
    const token = match[0];
    if (!token.trim()) continue;
    if ([",", "AND", "OR", "NOT"].includes(token)) {
      if (waitingForValue) return "Enter a value after the search field.";
      hasClause = false;
      operator = token;
      continue;
    }
    const field = /^-?([a-z]+)[:=](.*)$/iu.exec(token);
    if (field && fieldNames.has(field[1].toLowerCase())) {
      const name = field[1].toLowerCase();
      if (waitingForValue || (hasClause && name !== "sort" && name !== "group")) {
        return `Add AND or a comma before ${name}:.`;
      }
      hasClause = name !== "sort" && name !== "group";
      waitingForValue = !field[2];
    } else {
      if (!waitingForValue || hasClause) hasClause = true;
      waitingForValue = false;
    }
    operator = "";
  }
  if (waitingForValue) return "Enter a value after the search field.";
  if (operator === "OR" || operator === "NOT") return `Enter a value after ${operator}.`;
  return null;
}

export function searchSuggestions(query: string, view: ExplorerView, genreNames: readonly string[] = []) {
  const catalogFields = fields.filter(({ views }) => views.includes(viewCode[view]));
  let start = 0;
  let inherited: string | undefined;
  for (const match of query.matchAll(tokenPattern)) {
    const token = match[0];
    if (token === "," || token === "AND" || token === "OR" || token === "NOT") {
      if (token === "OR") {
        inherited = /^\s*-?([a-z]+)\s*[:=]/iu.exec(query.slice(start, match.index))?.[1] ?? inherited;
      } else {
        inherited = undefined;
      }
      start = match.index + token.length;
    }
  }
  const clause = query.slice(start);
  const value = /^(\s*-?\s*)(?:([a-z]+)\s*([:=])\s*)?("?[^"=,:]*)$/iu.exec(clause);
  if (value) {
    const [, leading, field, separator, input] = value;
    const name = (field ?? inherited ?? "").toLowerCase();
    const options = name === "country" && input.trim()
      ? countries.filter(({ iso }) => iso).map(({ name, code }) => code === input.toLowerCase() ? code.toUpperCase() : name)
      : values.get(name);
    if (options) {
      const prefix = input.replace(/^"/u, "").trim().toLowerCase();
      const candidates = name === "genre" && prefix
        ? matchGenreSuggestions(prefix, [...new Map([...options, ...genreNames].map((genre) => [genre.toLowerCase(), genre])).values()], 8)
        : options.filter((option) => option.toLowerCase().startsWith(prefix)).slice(0, 8);
      return {
        fields: catalogFields,
        matches: candidates.map((option) => ({
          label: option,
          query: query.slice(0, start) + leading + (field ? field + separator : "") + ((/[\s,:=]/u.test(option) || input.startsWith('"')) ? `"${option}"` : option),
        })),
      };
    }
    if (!field && !inherited && /^[a-z]+$/iu.test(input)) {
      return {
        fields: catalogFields,
        matches: catalogFields.filter(({ name }) => name.startsWith(input.toLowerCase())).slice(0, 8)
          .map(({ name }) => ({ label: `${name}:`, query: query.slice(0, start) + leading + name + ":" })),
      };
    }
  }
  return { fields: catalogFields, matches: [] };
}
