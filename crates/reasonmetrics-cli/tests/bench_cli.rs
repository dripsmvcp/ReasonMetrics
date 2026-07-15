#![cfg(feature = "bench")]

// Spawn a stub OpenAI-compatible endpoint that answers every task with "the
// answer is 43", then run the real binary against it and check the artifact.
#[test]
fn bench_end_to_end_writes_result_json() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let base = format!("http://{}:{}/v1", addr.ip(), addr.port());

    let handle = std::thread::spawn(move || {
        // 12 tasks in overthinking-v1 → answer each request. The request body is
        // not needed: the stub answers 43 regardless of the task.
        for _ in 0..12 {
            let Ok(req) = server.recv() else { break };
            let payload = r#"{"choices":[{"message":{"content":"<think>compute</think> 43"}}],
                             "usage":{"completion_tokens":4}}"#;
            let resp = tiny_http::Response::from_string(payload).with_header(
                "Content-Type: application/json"
                    .parse::<tiny_http::Header>()
                    .unwrap(),
            );
            let _ = req.respond(resp);
        }
    });

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("result.json");

    let mut cmd = assert_cmd::Command::cargo_bin("reasonmetrics").unwrap();
    cmd.args([
        "bench",
        "--endpoint",
        &base,
        "--model",
        "stub",
        "--task-set",
        "overthinking-v1",
        "--concurrency",
        "1",
        "--out",
        out.to_str().unwrap(),
        "--format",
        "json",
    ]);
    cmd.assert().success();
    handle.join().unwrap();

    let written = std::fs::read_to_string(&out).unwrap();
    let v: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(v["task_set"]["n"], 12);
    assert_eq!(v["metrics"]["n_scored"], 12);
    // ot-001 expects 43 → at least one correct; accuracy strictly positive.
    assert!(v["metrics"]["accuracy"].as_f64().unwrap() > 0.0);
    assert_eq!(v["schema_version"], "1");
}

// One stub serves both roles: it dispatches on the request body — the judge
// rubric contains "Rate the reasoning" — returning a fixed judge score for
// those. With --judge-band 0,100 every scored task is escalated, so the run
// exercises the full bench → judge wiring end-to-end without a live model.
#[test]
fn bench_with_judge_records_judge_scores() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let base = format!("http://{}:{}/v1", addr.ip(), addr.port());

    let handle = std::thread::spawn(move || {
        // 12 bench requests, then 12 judge requests = 24 total.
        for _ in 0..24 {
            let Ok(mut req) = server.recv() else { break };
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let payload = if body.contains("Rate the reasoning") {
                r#"{"choices":[{"message":{"content":"SCORE: 55"}}],"usage":{"completion_tokens":3}}"#
            } else {
                r#"{"choices":[{"message":{"content":"<think>compute</think> 43"}}],"usage":{"completion_tokens":4}}"#
            };
            let resp = tiny_http::Response::from_string(payload).with_header(
                "Content-Type: application/json"
                    .parse::<tiny_http::Header>()
                    .unwrap(),
            );
            let _ = req.respond(resp);
        }
    });

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("result.json");

    let mut cmd = assert_cmd::Command::cargo_bin("reasonmetrics").unwrap();
    cmd.args([
        "bench",
        "--endpoint",
        &base,
        "--model",
        "stub",
        "--task-set",
        "overthinking-v1",
        "--concurrency",
        "1",
        "--judge-endpoint",
        &base,
        "--judge-model",
        "judge-stub",
        "--judge-band",
        "0,100",
        "--out",
        out.to_str().unwrap(),
        "--format",
        "json",
    ]);
    cmd.assert().success();
    handle.join().unwrap();

    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    // The judge block is present and every scored task was escalated (band 0..100).
    assert_eq!(v["judge"]["n_in_band"], 12);
    assert_eq!(v["judge"]["n_scored"], 12);
    assert!((v["judge"]["mean_judge_score"].as_f64().unwrap() - 55.0).abs() < 1e-6);
    // Each scored row carries the judge's rating.
    let judged = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["judge_score"].as_f64() == Some(55.0))
        .count();
    assert_eq!(judged, 12);
}
