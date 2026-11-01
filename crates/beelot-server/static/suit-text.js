// Splits text into plain strings and suit-word parts so the page can draw a symbol next to each suit name.
// `words` maps a lower-case word to a suit key ("cœur" -> "hearts").
function splitSuitText(text, words) {
  const keys = Object.keys(words).sort((a, b) => b.length - a.length);
  if (!text) return [];
  if (!keys.length) return [text];
  const escaped = keys.map((word) => word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const pattern = new RegExp(`(?<![\\p{L}])(${escaped.join("|")})(?![\\p{L}])`, "giu");
  const parts = [];
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    if (match.index > last) parts.push(text.slice(last, match.index));
    parts.push({ suit: words[match[0].toLowerCase()], word: match[0] });
    last = match.index + match[0].length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts;
}

if (typeof module !== "undefined") module.exports = { splitSuitText };
