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
