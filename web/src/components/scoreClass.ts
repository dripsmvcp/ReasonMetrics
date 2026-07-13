// Shared red/amber/green threshold used by both the composite dial and the
// score-card bars: red < 50 / amber < 75 / green >= 75.
export function scoreClass(score: number): string {
  if (score < 50) return "score-red";
  if (score < 75) return "score-amber";
  return "score-green";
}
