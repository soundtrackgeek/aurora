export interface SelectionModifiers {
  ctrl: boolean;
  shift: boolean;
}

export interface WindowsSelection {
  selectedKeys: ReadonlySet<string>;
  anchorKey: string | null;
}

export function applyWindowsSelection(
  orderedKeys: readonly string[],
  currentKeys: ReadonlySet<string>,
  anchorKey: string | null,
  clickedKey: string,
  modifiers: SelectionModifiers,
): WindowsSelection {
  if (modifiers.shift && anchorKey) {
    const anchorIndex = orderedKeys.indexOf(anchorKey);
    const clickedIndex = orderedKeys.indexOf(clickedKey);
    if (anchorIndex >= 0 && clickedIndex >= 0) {
      const start = Math.min(anchorIndex, clickedIndex);
      const end = Math.max(anchorIndex, clickedIndex);
      const range = orderedKeys.slice(start, end + 1);
      return {
        selectedKeys: modifiers.ctrl ? new Set([...currentKeys, ...range]) : new Set(range),
        anchorKey,
      };
    }
  }

  if (modifiers.ctrl) {
    const selectedKeys = new Set(currentKeys);
    if (selectedKeys.has(clickedKey)) selectedKeys.delete(clickedKey);
    else selectedKeys.add(clickedKey);
    return { selectedKeys, anchorKey: clickedKey };
  }

  return { selectedKeys: new Set([clickedKey]), anchorKey: clickedKey };
}
