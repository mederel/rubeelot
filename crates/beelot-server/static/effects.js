// Decides the effects level ("full", "reduced" or "off") from what the browser stored and what the system asks for.
const EFFECT_LEVELS = ["full", "reduced", "off"];

function resolveEffectsLevel(stored, legacyReducedMotion, prefersReduced) {
  if (EFFECT_LEVELS.includes(stored)) return stored;
  if (legacyReducedMotion === "true") return "reduced";
  return prefersReduced ? "reduced" : "full";
}

if (typeof module !== "undefined") module.exports = { resolveEffectsLevel };
