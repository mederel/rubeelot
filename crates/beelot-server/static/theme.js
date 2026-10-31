// The table themes of Settings; "comic" is the default and needs no overrides in the stylesheet.
const THEMES = ["comic", "classic-green", "midnight", "sunset"];

function resolveTheme(stored) {
  return THEMES.includes(stored) ? stored : "comic";
}

if (typeof module !== "undefined") module.exports = { resolveTheme, THEMES };
