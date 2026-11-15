import { Fragment, type ReactNode } from "react";
import { parseBlocks } from "../assistantMarkdown";

/**
 * A small Markdown renderer for assistant answers: paragraphs, headings,
 * lists, tables, code blocks, and inline code, bold and italics. It builds
 * React elements only (never HTML strings), so model output cannot inject
 * markup. Inline code is offered as a search query when `onCode` is set.
 */
export function AssistantMarkdown({ text, onCode }: { text: string; onCode?: (code: string) => void }) {
  return <>{renderBlocks(text, onCode)}</>;
}

function renderBlocks(text: string, onCode?: (code: string) => void): ReactNode[] {
  return parseBlocks(text).map((block, key) => {
    switch (block.type) {
      case "heading": return <p key={key} className="assistant-md__heading">{renderInline(block.text, onCode)}</p>;
      case "code": return <pre key={key}><code>{block.text}</code></pre>;
      case "list": {
        const items = block.items.map((item, position) => <li key={position}>{renderInline(item, onCode)}</li>);
        return block.ordered ? <ol key={key}>{items}</ol> : <ul key={key}>{items}</ul>;
      }
      case "table": {
        const [head, ...body] = block.rows;
        return <div key={key} className="assistant-md__table"><table>
          {head && <thead><tr>{head.map((cell, position) => <th key={position}>{renderInline(cell, onCode)}</th>)}</tr></thead>}
          <tbody>{body.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, position) => <td key={position}>{renderInline(cell, onCode)}</td>)}</tr>)}</tbody>
        </table></div>;
      }
      case "paragraph": return <p key={key}>{block.lines.map((line, position) => <Fragment key={position}>{position > 0 && <br />}{renderInline(line, onCode)}</Fragment>)}</p>;
    }
  });
}

const inlinePattern = /(`[^`]+`|\*\*[^*]+\*\*|__[^_]+__|\*[^*\s][^*]*\*|\[[^\]]+\]\([^)]+\))/g;

function renderInline(text: string, onCode?: (code: string) => void): ReactNode[] {
  return text.split(inlinePattern).filter(Boolean).map((piece, key) => {
    if (piece.startsWith("`") && piece.endsWith("`") && piece.length > 2) {
      const code = piece.slice(1, -1);
      return onCode
        ? <button key={key} type="button" className="assistant-md__query" title="Use in Aurora’s search" onClick={() => onCode(code)}><code>{code}</code></button>
        : <code key={key}>{code}</code>;
    }
    if ((piece.startsWith("**") && piece.endsWith("**")) || (piece.startsWith("__") && piece.endsWith("__"))) return <strong key={key}>{piece.slice(2, -2)}</strong>;
    if (piece.startsWith("*") && piece.endsWith("*") && piece.length > 2) return <em key={key}>{piece.slice(1, -1)}</em>;
    const link = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(piece);
    if (link) return <span key={key}>{link[1]}</span>;
    return <Fragment key={key}>{piece}</Fragment>;
  });
}
