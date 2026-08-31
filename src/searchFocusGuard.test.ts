import { afterEach, describe, expect, it } from "vitest";
import { preparePopulatedInputForFocus } from "./searchFocusGuard";

afterEach(() => {
  document.body.replaceChildren();
});

describe("Windows WebView search focus guard", () => {
  it("empties a populated input until native focus has completed, then restores it", () => {
    const input = document.createElement("input");
    input.value = "year:1986 NOT genre:scores OR soundtrack";
    document.body.append(input);
    const scheduled: Array<() => void> = [];

    expect(preparePopulatedInputForFocus(input, true, (restore) => scheduled.push(restore))).toBe(true);
    expect(input.value).toBe("");

    input.focus();
    scheduled[0]();

    expect(input.value).toBe("year:1986 NOT genre:scores OR soundtrack");
    expect(input.selectionStart).toBe(input.value.length);
    expect(input.selectionEnd).toBe(input.value.length);
  });

  it("does not disturb empty, already focused, or non-WebView inputs", () => {
    const input = document.createElement("input");
    document.body.append(input);
    const schedule = () => { throw new Error("restore should not be scheduled"); };

    expect(preparePopulatedInputForFocus(input, true, schedule)).toBe(false);
    input.value = "existing search";
    input.focus();
    expect(preparePopulatedInputForFocus(input, true, schedule)).toBe(false);
    input.blur();
    expect(preparePopulatedInputForFocus(input, false, schedule)).toBe(false);
    expect(input.value).toBe("existing search");
  });

  it("does not overwrite text entered before the restore frame", () => {
    const input = document.createElement("input");
    input.value = "stored search";
    document.body.append(input);
    let restore: () => void = () => undefined;

    preparePopulatedInputForFocus(input, true, (scheduled) => { restore = scheduled; });
    input.value = "new text";
    restore();

    expect(input.value).toBe("new text");
  });
});
