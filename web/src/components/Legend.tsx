// Legend bar for the annotated thinking text: restart / verification /
// repetition swatches.

export function Legend() {
  return (
    <div className="legend">
      <LegendItem swatchClass="ann-restart" label="restart ⟲" />
      <LegendItem swatchClass="ann-verification" label="verification" />
      <LegendItem swatchClass="rep-pill" label="repetition ×N" />
    </div>
  );
}

function LegendItem({ swatchClass, label }: { swatchClass: string; label: string }) {
  return (
    <span className="legend-item">
      <span className={`legend-swatch ${swatchClass}`}>{label}</span>
    </span>
  );
}
