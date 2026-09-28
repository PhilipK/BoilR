import { convertFileSrc } from "@tauri-apps/api/core";

export const errorMessage = (err: unknown): string => {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return "Unknown error";
};

/** A displayable URL for a local file path; web and data URLs pass through unchanged. */
export const toImageSrc = (icon?: string | null, version?: number): string | null => {
  if (!icon) return null;
  if (/^(https?:|data:)/.test(icon)) return icon;
  try {
    const src = convertFileSrc(icon);
    return version ? `${src}?v=${version}` : src;
  } catch {
    return null;
  }
};

/** True when a launcher error just means the launcher isn't installed here. */
export const isNotInstalledError = (message: string): boolean =>
  /no such file|not found|cannot find|could not find|os error 2|os error 3/i.test(message);

/** A one-line explanation of a launcher error in plain words. */
export const friendlyError = (message: string): string => {
  if (isNotInstalledError(message)) return "Not found on this PC";
  if (/eof while parsing|expected value|invalid type|parse/i.test(message))
    return "Found it, but couldn't read its game list";
  if (/permission denied|os error 13/i.test(message)) return "BoilR isn't allowed to read its files";
  return "Couldn't read its games";
};

/** Two letters for a game's placeholder tile. */
export const initials = (name: string): string => {
  const words = name.replace(/[^\p{L}\p{N} ]/gu, " ").split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
};

const TILE_COLORS = ["bg-dusk", "bg-mauve", "bg-harbour", "bg-ember"];

/** A stable tile colour per game, so the list has rhythm without random noise. */
export const tileColor = (name: string): string => {
  let hash = 0;
  for (const ch of name) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return TILE_COLORS[hash % TILE_COLORS.length];
};

export const plural = (n: number, one: string, many = `${one}s`): string =>
  `${n} ${n === 1 ? one : many}`;
