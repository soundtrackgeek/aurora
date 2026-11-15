/** Splits assistant Markdown into the blocks `AssistantMarkdown` renders. */
export type Block =
  | { type: "paragraph"; lines: string[] }
  | { type: "heading"; text: string }
  | { type: "list"; ordered: boolean; items: string[] }
  | { type: "table"; rows: string[][] }
  | { type: "code"; text: string };

export function parseBlocks(text: string): Block[] {
  const blocks: Block[] = [];
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  let index = 0;
  while (index < lines.length) {
    const line = lines[index];
    const trimmed = line.trim();
    if (!trimmed) { index += 1; continue; }
    if (trimmed.startsWith("```")) {
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index].trim().startsWith("```")) { code.push(lines[index]); index += 1; }
      index += 1;
      blocks.push({ type: "code", text: code.join("\n") });
      continue;
    }
    const heading = /^#{1,6}\s+(.*)$/.exec(trimmed);
    if (heading) { blocks.push({ type: "heading", text: heading[1] }); index += 1; continue; }
    if (trimmed.startsWith("|")) {
      const rows: string[][] = [];
      while (index < lines.length && lines[index].trim().startsWith("|")) {
        const cells = lines[index].trim().replace(/^\||\|$/g, "").split("|").map((cell) => cell.trim());
        if (!cells.every((cell) => /^:?-{2,}:?$/.test(cell))) rows.push(cells);
        index += 1;
      }
      blocks.push({ type: "table", rows });
      continue;
    }
    const bullet = /^\s*(?:[-*•]|(\d+)[.)])\s+(.*)$/.exec(line);
    if (bullet) {
      const ordered = bullet[1] !== undefined;
      const items: string[] = [];
      while (index < lines.length) {
        const item = /^\s*(?:[-*•]|(\d+)[.)])\s+(.*)$/.exec(lines[index]);
        if (item) { items.push(item[2]); index += 1; continue; }
        // A wrapped continuation line belongs to the previous item.
        if (lines[index].trim() && /^\s{2,}/.test(lines[index]) && items.length) {
          items[items.length - 1] += ` ${lines[index].trim()}`; index += 1; continue;
        }
        break;
      }
      blocks.push({ type: "list", ordered, items });
      continue;
    }
    const paragraph: string[] = [];
    while (index < lines.length && lines[index].trim() && !/^(```|#{1,6}\s|\||\s*(?:[-*•]|\d+[.)])\s)/.test(lines[index].trim())) {
      paragraph.push(lines[index].trim()); index += 1;
    }
    if (!paragraph.length) { paragraph.push(trimmed); index += 1; }
    blocks.push({ type: "paragraph", lines: paragraph });
  }
  return blocks;
}
