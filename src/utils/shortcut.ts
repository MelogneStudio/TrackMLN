const MODIFIER_CODES = new Set([
  "AltLeft",
  "AltRight",
  "ControlLeft",
  "ControlRight",
  "MetaLeft",
  "MetaRight",
  "ShiftLeft",
  "ShiftRight"
]);

export function buildShortcut(event: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(event.code)) {
    return null;
  }

  const parts: string[] = [];
  if (event.ctrlKey) parts.push("control");
  if (event.altKey) parts.push("alt");
  if (event.shiftKey) parts.push("shift");
  if (event.metaKey) parts.push("super");
  parts.push(normalizeKey(event));

  return parts.join("+");
}

function normalizeKey(event: KeyboardEvent): string {
  if (event.code.startsWith("Key") || event.code.startsWith("Digit")) {
    return event.code.slice(event.code.startsWith("Key") ? 3 : 5);
  }

  switch (event.code) {
    case "Space":
      return "Space";
    case "Backquote":
      return "`";
    case "Minus":
      return "-";
    case "Equal":
      return "=";
    case "BracketLeft":
      return "[";
    case "BracketRight":
      return "]";
    case "Backslash":
      return "\\";
    case "Semicolon":
      return ";";
    case "Quote":
      return "'";
    case "Comma":
      return ",";
    case "Period":
      return ".";
    case "Slash":
      return "/";
    default:
      return event.key.length === 1 ? event.key.toUpperCase() : event.key;
  }
}

/** "control+shift+Space" -> ["Ctrl", "Shift", "Space"] */
export function shortcutParts(shortcut: string): string[] {
  return shortcut.split("+").map((part) => {
    switch (part.toLowerCase()) {
      case "control":
      case "ctrl":
        return "Ctrl";
      case "shift":
        return "Shift";
      case "alt":
        return "Alt";
      case "super":
        return "Win";
      case "space":
        return "Space";
      default:
        return part.length === 1 ? part.toUpperCase() : part;
    }
  });
}

export function formatShortcut(shortcut: string): string {
  return shortcutParts(shortcut).join(" + ");
}
