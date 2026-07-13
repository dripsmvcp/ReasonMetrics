use minijinja::{context, Environment};
use std::fs;
use std::path::Path;

use reasonmetrics_core::trace::ScoredTrace;

fn auto_escape(name: &str) -> minijinja::AutoEscape {
    if name.ends_with(".html") || name == "report" {
        minijinja::AutoEscape::Html
    } else {
        minijinja::AutoEscape::None
    }
}

struct DimensionStats {
    name: String,
    avg: f32,
    pct_high: f32,
    css_class: String,
}

pub fn generate_report(scored: &[ScoredTrace], path: &Path) -> anyhow::Result<()> {
    if scored.is_empty() {
        anyhow::bail!("No traces to report on");
    }
    let total = scored.len();
    let avg_quality: f32 = scored.iter().map(|s| s.quality_score).sum::<f32>() / total as f32;

    let dimensions = compute_dimension_stats(scored);
    let mut sorted: Vec<_> = scored.iter().collect();
    sorted.sort_by(|a, b| a.quality_score.total_cmp(&b.quality_score));
    let worst_10: Vec<_> = sorted.iter().take(10).collect();

    let worst_traces_data: Vec<_> = worst_10
        .iter()
        .map(|t| {
            let mut issues = Vec::new();
            if t.efficiency_score < 50.0 {
                issues.push("Excessive restarts/backtracking".to_string());
            }
            if t.language_score < 70.0 {
                issues.push("Language mixing detected".to_string());
            }
            if t.answer_alignment_score < 50.0 {
                issues.push("Answer not at end of trace".to_string());
            }
            if t.repetition_score < 50.0 {
                issues.push("High repetition".to_string());
            }
            if t.overthinking_score < 50.0 {
                issues.push("Overthinking (trace too long for problem)".to_string());
            }
            if t.verification_score < 40.0 {
                issues.push("No self-verification".to_string());
            }
            let thinking_preview: String = t.thinking.chars().take(500).collect();
            context!(
                id => t.id,
                quality_score => format!("{:.1}", t.quality_score),
                issues => issues,
                thinking_preview => thinking_preview,
            )
        })
        .collect();
    let dim_rows: Vec<_> = dimensions
        .iter()
        .map(|d| {
            context!(
                name => d.name,
                avg => format!("{:.1}", d.avg),
                pct_high => format!("{:.1}", d.pct_high),
                css_class => d.css_class,
            )
        })
        .collect();
    let high_quality = scored.iter().filter(|s| s.quality_score >= 70.0).count();
    let language_mixed = scored.iter().filter(|s| s.is_language_mixed).count();
    let no_verification = scored.iter().filter(|s| !s.has_self_verification).count();
    let mut env = Environment::new();
    env.set_auto_escape_callback(auto_escape);
    env.add_template("report", REPORT_TEMPLATE)?;
    let tmpl = env.get_template("report")?;
    let html = tmpl.render(context!(
        total => total,
        avg_quality => format!("{:.1}", avg_quality),
        high_quality => high_quality,
        high_quality_pct => format!("{:.1}", high_quality as f32 / total as f32 * 100.0),
        language_mixed => language_mixed,
        no_verification => no_verification,
        dimensions => dim_rows,
        worst_traces => worst_traces_data,
    ))?;
    fs::write(path, html)?;
    Ok(())
}

type DimExtractor<'a> = Vec<(&'a str, Box<dyn Fn(&ScoredTrace) -> f32>)>;

fn compute_dimension_stats(scored: &[ScoredTrace]) -> Vec<DimensionStats> {
    let n = scored.len() as f32;
    let dims: DimExtractor = vec![
        ("Efficiency", Box::new(|s: &ScoredTrace| s.efficiency_score)),
        (
            "Language Consistency",
            Box::new(|s: &ScoredTrace| s.language_score),
        ),
        (
            "Answer Alignment",
            Box::new(|s: &ScoredTrace| s.answer_alignment_score),
        ),
        (
            "Structural Clarity",
            Box::new(|s: &ScoredTrace| s.structural_score),
        ),
        ("Repetition", Box::new(|s: &ScoredTrace| s.repetition_score)),
        (
            "Overthinking",
            Box::new(|s: &ScoredTrace| s.overthinking_score),
        ),
        (
            "Self-Verification",
            Box::new(|s: &ScoredTrace| s.verification_score),
        ),
        (
            "Length Calibration",
            Box::new(|s: &ScoredTrace| s.length_score),
        ),
    ];

    dims.into_iter()
        .map(|(name, getter)| {
            let avg: f32 = scored.iter().map(&getter).sum::<f32>() / n;
            let high = scored.iter().filter(|s| getter(s) >= 80.0).count();
            let pct_high = high as f32 / n * 100.0;
            let css_class = if avg >= 80.0 {
                "good"
            } else if avg >= 50.0 {
                "okay"
            } else {
                "bad"
            };
            DimensionStats {
                name: name.to_string(),
                avg,
                pct_high,
                css_class: css_class.to_string(),
            }
        })
        .collect()
}

/// Self-contained HTML template with inline CSS. No external dependencies.
const REPORT_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>reasonmetrics quality report</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
         max-width: 1100px; margin: 2rem auto; padding: 0 1.5rem; color: #1a1a1a;
         background: #fafafa; line-height: 1.6; }
  h1 { font-size: 1.8rem; margin-bottom: 0.5rem; }
  h2 { font-size: 1.3rem; margin: 2rem 0 1rem; border-bottom: 2px solid #e5e7eb; padding-bottom: 0.5rem; }
  .summary { display: flex; gap: 1.5rem; flex-wrap: wrap; margin: 1.5rem 0; }
  .stat-card { background: white; border: 1px solid #e5e7eb; border-radius: 8px;
               padding: 1rem 1.5rem; flex: 1; min-width: 150px; }
  .stat-card .value { font-size: 1.5rem; font-weight: 700; }
  .stat-card .label { font-size: 0.85rem; color: #6b7280; }
  table { width: 100%; border-collapse: collapse; background: white;
          border: 1px solid #e5e7eb; border-radius: 8px; overflow: hidden; }
  th, td { padding: 10px 14px; text-align: left; border-bottom: 1px solid #f0f0f0; }
  th { background: #f9fafb; font-weight: 600; font-size: 0.85rem; color: #374151; }
  .bar-bg { background: #e5e7eb; height: 18px; border-radius: 9px; overflow: hidden; }
  .bar-fill { height: 100%; border-radius: 9px; }
  .good { background: #22c55e; }
  .okay { background: #f59e0b; }
  .bad  { background: #ef4444; }
  .trace-card { background: white; border: 1px solid #e5e7eb; border-radius: 8px;
                padding: 1rem; margin-bottom: 1rem; }
  .trace-card .header { display: flex; justify-content: space-between; margin-bottom: 0.5rem; }
  .trace-card .id { font-weight: 600; }
  .trace-card .score { font-weight: 700; }
  .trace-card .issues { list-style: none; padding: 0; margin: 0.5rem 0; }
  .trace-card .issues li { color: #dc2626; font-size: 0.85rem; }
  .trace-card .issues li::before { content: "⚠ "; }
  .trace-preview { background: #f9fafb; padding: 0.75rem; border-radius: 6px;
                   font-family: 'SF Mono', Consolas, monospace; font-size: 0.8rem;
                   white-space: pre-wrap; word-break: break-word; max-height: 150px;
                   overflow-y: auto; color: #4b5563; border: 1px solid #e5e7eb; }
  .footer { margin-top: 3rem; padding: 1rem 0; border-top: 1px solid #e5e7eb;
            font-size: 0.8rem; color: #9ca3af; }
</style>
</head>
<body>

<h1>reasonmetrics quality report</h1>
<p style="color:#6b7280">{{ total }} traces analyzed</p>

<div class="summary">
  <div class="stat-card">
    <div class="value">{{ avg_quality }}</div>
    <div class="label">Avg Quality Score</div>
  </div>
  <div class="stat-card">
    <div class="value">{{ high_quality }} <span style="font-size:0.9rem;color:#6b7280">({{ high_quality_pct }}%)</span></div>
    <div class="label">High Quality (≥70)</div>
  </div>
  <div class="stat-card">
    <div class="value">{{ language_mixed }}</div>
    <div class="label">Language Mixed</div>
  </div>
  <div class="stat-card">
    <div class="value">{{ no_verification }}</div>
    <div class="label">No Self-Verification</div>
  </div>
</div>

<h2>Score Distribution by Dimension</h2>
<table>
<tr><th>Dimension</th><th>Average</th><th style="width:40%">Distribution</th><th>% High (≥80)</th></tr>
{% for dim in dimensions %}
<tr>
  <td>{{ dim.name }}</td>
  <td>{{ dim.avg }}/100</td>
  <td><div class="bar-bg"><div class="bar-fill {{ dim.css_class }}" style="width:{{ dim.avg }}%"></div></div></td>
  <td>{{ dim.pct_high }}%</td>
</tr>
{% endfor %}
</table>

<h2>Top 10 Lowest Quality Traces</h2>
{% for trace in worst_traces %}
<div class="trace-card">
  <div class="header">
    <span class="id">ID: {{ trace.id }}</span>
    <span class="score" style="color:#dc2626">{{ trace.quality_score }}/100</span>
  </div>
  <ul class="issues">
    {% for issue in trace.issues %}<li>{{ issue }}</li>{% endfor %}
  </ul>
  <div class="trace-preview">{{ trace.thinking_preview }}...</div>
</div>
{% endfor %}

<div class="footer">
  Generated by <strong>reasonmetrics</strong> — Reasoning trace quality auditor
</div>

</body>
</html>"#;
