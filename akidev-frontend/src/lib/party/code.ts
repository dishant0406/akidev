// The party code: six letters and digits, as the server makes them (the ones that are not easily
// mistaken for each other).

export const CODE_LENGTH = 6;

/** What is left of `text` once it is made of code characters only, at most one code long. */
function clean(text: string): string {
  return text.toUpperCase().replace(/[^A-HJ-NP-Z2-9]/g, "").slice(0, CODE_LENGTH);
}

/**
 * The code in what was typed or pasted: the `party=` of a link, or else the letters. A pasted link
 * must not be read as a code made of its own letters.
 */
export function partyCodeFrom(text: string): string {
  const link = /[?&]party=([^&#\s]*)/i.exec(text);
  if (link) return clean(link[1]);
  return /^\s*https?:\/\//i.test(text) ? "" : clean(text);
}
