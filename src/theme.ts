export const THEMES = [
  { id: "midnight", label: "Modern Dark" },
  { id: "daylight", label: "Minimal White" },
  { id: "retro", label: "Classic Register" },
  { id: "ocean", label: "Emerald Teal" },
  { id: "blue", label: "Modern Blue" },
  { id: "navy", label: "Midnight Navy" },
  { id: "black", label: "Black Professional" },
  { id: "red", label: "Red Retail" },
  { id: "slate", label: "Slate Gray" },
  { id: "contrast", label: "High Contrast" },
] as const;
export type ThemeId = (typeof THEMES)[number]["id"];

export function applyTheme(id: string | undefined) {
  const theme = THEMES.some((t) => t.id === id) ? (id as ThemeId) : "midnight";
  document.documentElement.dataset.theme = theme;
}
