// Shared red/amber/green threshold used by both the composite dial and the
// score-card bars: red < 50 / amber < 75 / green >= 75.
//
// On the composite these bands are percentiles against a reference corpus of
// real reasoning traces, so they read as: red = below the median trace, amber =
// above median, green = top quartile. (Before the #30 calibration the composite
// was a raw score where 99.9% of real traces cleared 70, so essentially
// everything rendered green — including a trace that never answers.)
//
// The per-dimension bars are still RAW scores, which are saturated (language is
// exactly 100 for 98.1% of real traces). Green on a dimension bar is therefore a
// much weaker claim than green on the dial.
export function scoreClass(score: number): string {
  if (score < 50) return "score-red";
  if (score < 75) return "score-amber";
  return "score-green";
}
