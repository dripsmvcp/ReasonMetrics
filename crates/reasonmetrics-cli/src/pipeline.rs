// CLI scoring wrapper: core's pure scoring plus a terminal progress bar.

use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use reasonmetrics_core::config::ScoringConfig;
use reasonmetrics_core::scorers::build_scorers;
use reasonmetrics_core::scoring::score_one;
use reasonmetrics_core::trace::{ScoredTrace, TraceRecord};

pub fn score_traces(traces: &[TraceRecord], scoring_config: &ScoringConfig) -> Vec<ScoredTrace> {
    let scorers = build_scorers(scoring_config);

    let pb = ProgressBar::new(traces.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} ({per_sec}) {msg}")
            .expect("Invalid progress bar template"),
    );

    let results: Vec<ScoredTrace> = traces
        .par_iter()
        .map(|trace| {
            let scored = score_one(trace, &scorers);
            pb.inc(1);
            scored
        })
        .collect();

    pb.finish_with_message("Done");
    results
}
