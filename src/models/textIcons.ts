// `{icon:N}` / `{dyn:N}` are the mining pipeline's resolutions of the game's
// `<d>` inline elements; each renders as its best text equivalent or drops out.
const MARKER = /\{(icon|dyn):(\d+)\}/g;

// The style-rank pip; the sprite is a four-pointed sparkle.
const RANK_PIP = 1700;
const RANK_GLYPH = "◆";

// 4 is the primary attack button, 3 the secondary.
const BUTTON_TEXT: Record<number, string> = {
  4: "X",
  3: "Y",
};

export const textIconGlyph = (id: number): string => {
  if (id === RANK_PIP) return RANK_GLYPH;
  return BUTTON_TEXT[id] ?? "";
};

export const resolveTextIcons = (text: string): string => {
  if (!text || !text.includes("{")) return text;
  const out = text.replace(MARKER, (_match, kind: string, digits: string) =>
    kind === "icon" ? textIconGlyph(Number(digits)) : ""
  );
  return out
    .replace(/ {2,}/g, " ")
    .replace(/ +([,.:;)])/g, "$1")
    .trim();
};
