# Security Policy

## Reporting a vulnerability

Please report vulnerabilities privately via **GitHub Security Advisories**
("Report a vulnerability" on the repository's Security tab). You'll get a
response within 72 hours. Please don't open public issues for security reports.

## Scope notes

ReasonMetrics has no server, no accounts, and no telemetry — the web analyzer
runs entirely client-side — so there is no hosted infrastructure to attack.
The surfaces that DO matter:

- **Share links**: traces are packed into the URL fragment (`#t=`). Anything
  that makes fragment decoding execute or leak content is in scope.
- **The wasm boundary**: malformed input to `analyze()` should fail closed.
- **CLI input parsing**: untrusted `.jsonl` / `.jsonl.gz` files must not be
  able to do more than produce an error.
- **CI workflows**: the registry accepts external data PRs; anything that lets
  a data file influence workflow execution is in scope.

## Supported versions

The latest release (and `main`) receive fixes. There are no backports.
