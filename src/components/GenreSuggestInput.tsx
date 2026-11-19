import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type ChangeEvent, type InputHTMLAttributes, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { MAX_GENRE_SUGGESTIONS, matchGenreSuggestions } from "../genreMatch";

type GenreSuggestInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange" | "list"> & {
  value: string;
  suggestions: readonly string[];
  onChange: (event: ChangeEvent<HTMLInputElement>) => void;
};

interface ListPosition { left: number; top: number; width: number; maxHeight: number; above: boolean }

/** A text input with Aurora's own genre suggestions, so matching does not depend on the web engine's datalist. */
export function GenreSuggestInput({ value, suggestions, onChange, onKeyDown, onBlur, onFocus, disabled, ...inputProps }: GenreSuggestInputProps) {
  const listId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const choosingRef = useRef(false);
  const [open, setOpen] = useState(false);
  const [showAll, setShowAll] = useState(false);
  const [active, setActive] = useState(-1);
  const [position, setPosition] = useState<ListPosition | null>(null);

  const matches = useMemo(() => {
    if (showAll && !value.trim()) return suggestions.slice(0, MAX_GENRE_SUGGESTIONS * 5);
    const found = matchGenreSuggestions(value, suggestions);
    return found.length === 1 && found[0] === value ? [] : found;
  }, [showAll, suggestions, value]);
  const visible = open && !disabled && matches.length > 0;

  const place = useCallback(() => {
    const input = inputRef.current;
    if (!input) return;
    const rect = input.getBoundingClientRect();
    const below = window.innerHeight - rect.bottom - 8;
    const aboveSpace = rect.top - 8;
    const above = below < 160 && aboveSpace > below;
    setPosition({
      left: rect.left,
      top: above ? rect.top - 2 : rect.bottom + 2,
      width: rect.width,
      maxHeight: Math.max(80, Math.min(260, above ? aboveSpace : below)),
      above,
    });
  }, []);

  useLayoutEffect(() => { if (visible) place(); }, [place, visible, matches.length]);
  useEffect(() => {
    if (!visible) return undefined;
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [place, visible]);

  const choose = (genre: string) => {
    const input = inputRef.current;
    setOpen(false);
    setShowAll(false);
    setActive(-1);
    if (!input) return;
    // Go through the native setter so React sees a normal change event.
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    choosingRef.current = true;
    try {
      setter?.call(input, genre);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    } finally {
      choosingRef.current = false;
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    onKeyDown?.(event);
    if (event.defaultPrevented) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open || (!value.trim() && !showAll)) {
        setOpen(true);
        setShowAll(true);
        setActive(0);
        return;
      }
      if (!matches.length) return;
      const step = event.key === "ArrowDown" ? 1 : -1;
      setActive((current) => (current + step + matches.length) % matches.length);
    } else if (event.key === "Enter" && visible && active >= 0 && active < matches.length) {
      event.preventDefault();
      choose(matches[active]);
    } else if (event.key === "Escape" && visible) {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
    }
  };

  useEffect(() => {
    if (!visible || active < 0) return;
    document.getElementById(`${listId}-${active}`)?.scrollIntoView({ block: "nearest" });
  }, [active, listId, visible]);

  return (
    <>
      <input
        {...inputProps}
        ref={inputRef}
        type="text"
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={visible}
        aria-controls={visible ? listId : undefined}
        aria-activedescendant={visible && active >= 0 ? `${listId}-${active}` : undefined}
        autoComplete="off"
        disabled={disabled}
        value={value}
        onChange={(event) => {
          onChange(event);
          if (choosingRef.current) return;
          setOpen(true);
          setShowAll(false);
          setActive(-1);
        }}
        onFocus={onFocus}
        onBlur={(event) => {
          setOpen(false);
          setShowAll(false);
          onBlur?.(event);
        }}
        onKeyDown={handleKeyDown}
      />
      {visible && position ? createPortal(
        <ul
          id={listId}
          role="listbox"
          className="genre-suggest__list"
          style={{
            left: position.left,
            width: position.width,
            maxHeight: position.maxHeight,
            ...(position.above ? { bottom: window.innerHeight - position.top } : { top: position.top }),
          }}
        >
          {matches.map((genre, index) => (
            <li
              id={`${listId}-${index}`}
              key={genre}
              role="option"
              aria-selected={index === active}
              className={index === active ? "is-active" : undefined}
              onMouseDown={(event) => { event.preventDefault(); choose(genre); }}
              onMouseEnter={() => setActive(index)}
            >
              {genre}
            </li>
          ))}
        </ul>,
        document.body,
      ) : null}
    </>
  );
}
