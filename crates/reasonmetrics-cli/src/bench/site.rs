//! Render assembled leaderboard groups into a single self-contained static page.
//!
//! No external CSS, JS, fonts, or images — the page is one file that opens
//! anywhere and matches the project's no-server, no-telemetry stance. Output is
//! deterministic (no wall-clock), so a regenerated `leaderboard/index.html` only
//! diffs when the underlying results change.

use crate::bench::leaderboard::{Group, Row};

const STYLE: &str = "\
:root{color-scheme:light dark}\
*{box-sizing:border-box}\
body{margin:0;font:15px/1.5 system-ui,-apple-system,Segoe UI,Roboto,sans-serif;\
padding:2rem 1rem;max-width:60rem;margin:0 auto;color:#1a1a1a;background:#fff}\
@media(prefers-color-scheme:dark){body{color:#e6e6e6;background:#141414}\
a{color:#8ab4f8}th{border-color:#333!important}td{border-color:#262626!important}\
.set{background:#1c1c1c!important}}\
h1{font-size:1.6rem;margin:0 0 .25rem}\
h2{font-size:1.15rem;margin:2rem 0 .5rem}\
p.lead{color:#666;margin:.25rem 0 1.5rem}\
table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}\
th,td{padding:.4rem .6rem;text-align:right;border-bottom:1px solid #e6e6e6}\
th:first-child,td:first-child{text-align:left}\
th{font-weight:600;border-bottom:2px solid #ccc}\
tr.set td{font-weight:600;background:#f5f5f5;text-align:left}\
.muted{color:#888}\
footer{margin-top:2.5rem;padding-top:1rem;border-top:1px solid #ccc;color:#666;font-size:.9rem}\
code{font:13px/1.4 ui-monospace,Menlo,Consolas,monospace}";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn fmt_opt(v: Option<f32>, decimals: usize) -> String {
    v.map(|x| format!("{x:.*}", decimals))
        .unwrap_or_else(|| "-".into())
}

/// Civil date (UTC) from a Unix timestamp — Howard Hinnant's algorithm, so the
/// site needs no date crate. Returns `YYYY-MM-DD`, or `-` for a zero stamp.
fn ymd(secs: u64) -> String {
    if secs == 0 {
        return "-".into();
    }
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn row_html(r: &Row) -> String {
    format!(
        "<tr><td>{}</td><td>{}</td><td>{:.1}</td><td>{:.1}%</td><td>{}</td>\
         <td>{}</td><td>{}</td><td class=\"muted\">{}</td><td class=\"muted\">{}</td></tr>\n",
        esc(&r.model),
        r.samples,
        r.quality,
        r.accuracy * 100.0,
        fmt_opt(r.tokens_per_correct, 0),
        fmt_opt(r.cost_per_1k_correct, 2),
        r.n_scored,
        esc(&r.tool_version),
        ymd(r.generated_at),
    )
}

fn group_html(g: &Group) -> String {
    let mut s = format!(
        "<h2>{} <span class=\"muted\">· {} tasks · sha {}</span></h2>\n\
         <table>\n<thead><tr>\
         <th>model</th><th>samples</th><th>quality</th><th>accuracy</th>\
         <th>tokens/correct</th><th>cost/1k</th><th>n</th><th>tool</th><th>measured</th>\
         </tr></thead>\n<tbody>\n",
        esc(&g.task_set),
        g.n,
        esc(&g.sha256[..g.sha256.len().min(8)]),
    );
    for r in &g.rows {
        s.push_str(&row_html(r));
    }
    s.push_str("</tbody></table>\n");
    s
}

/// A complete standalone HTML page for the given leaderboard groups.
pub fn render(groups: &[Group]) -> String {
    let mut body = String::new();
    if groups.is_empty() {
        body.push_str("<p class=\"lead\">No results yet.</p>\n");
    }
    for g in groups {
        body.push_str(&group_html(g));
    }

    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
         <title>ReasonMetrics — Overthinking Leaderboard</title>\n<style>{STYLE}</style>\n\
         </head>\n<body>\n\
         <h1>ReasonMetrics — Overthinking Leaderboard</h1>\n\
         <p class=\"lead\">Reasoning quality, answer accuracy, and token cost for models run \
         over a fixed, content-hashed task set. <strong>quality</strong> is the calibrated \
         composite (percentile vs real traces); <strong>accuracy</strong> is a normalized \
         answer match (pass@k when samples &gt; 1); <strong>tokens/correct</strong> counts \
         every sample's tokens over the number solved. Higher quality and accuracy are better; \
         lower tokens/correct is better.</p>\n\
         {body}\
         <footer>\n\
         <p>Every row is a committed result JSON plus the exact command that produced it, so \
         each entry is a reviewable pull request. Rows are grouped by task set (accuracy across \
         different sets is not comparable) and the newest run wins per model. No accounts, no \
         telemetry — traces never leave the machine they run on.</p>\n\
         <p>Reproduce or submit: see <code>docs/BENCH.md</code>. Generated by \
         <code>reasonmetrics leaderboard --site</code>.</p>\n\
         </footer>\n</body>\n</html>\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::leaderboard::{Group, Row};

    fn group() -> Group {
        Group {
            task_set: "overthinking-v2".into(),
            sha256: "34ae22b1a7b9".into(),
            n: 100,
            rows: vec![Row {
                model: "qwen3:1.7b".into(),
                samples: 4,
                quality: 63.9,
                accuracy: 0.89,
                tokens_per_correct: Some(210.0),
                cost_per_1k_correct: None,
                n_scored: 100,
                tool_version: "0.2.0".into(),
                generated_at: 1_752_537_600, // 2025-07-15
            }],
        }
    }

    #[test]
    fn renders_a_standalone_page() {
        let html = render(&[group()]);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<style>"), "CSS is inlined");
        assert!(html.contains("overthinking-v2"));
        assert!(html.contains("qwen3:1.7b"));
        assert!(html.contains("34ae22b1"), "short sha shown");
        // Self-contained: no external asset references.
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert!(!html.contains("src="));
    }

    #[test]
    fn escapes_model_names() {
        let mut g = group();
        g.rows[0].model = "<script>x</script>".into();
        let html = render(&[g]);
        assert!(!html.contains("<script>x"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn ymd_converts_known_dates() {
        assert_eq!(ymd(0), "-");
        assert_eq!(ymd(1_752_537_600), "2025-07-15");
        assert_eq!(ymd(946_684_800), "2000-01-01");
    }

    #[test]
    fn empty_groups_still_render() {
        let html = render(&[]);
        assert!(html.contains("No results yet"));
    }
}
