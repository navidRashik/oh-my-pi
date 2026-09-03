# 0009. Output is bounded centrally; full results become artifacts

Status: accepted
Date: 2026-09-02
Area: runtime

## Context

A raw tool callback in pi and omp v1 returns an arbitrary amount of text. Both codebases grew
helpers around that fact — a truncation utility, a `maxBytes` default, a formatted notice — and left
them optional. The result was a distribution of near-duplicates: each tool author who remembered
wrote a slightly different notice, in prose, appended to the data:

```ts
text += `\n${theme.fg("warning", `[Truncated: ${truncation.outputLines} lines shown (${formatSize(truncation.maxBytes ?? DEFAULT_MAX_BYTES)} limit)]`)}`;
```

Three failures recur from optional bounding:

1. Tool authors each invent a truncation notice; the model learns none of them reliably.
2. The notice is mixed into the data. The model has to guess where tool output ends and harness
   commentary begins; a themed ANSI string is not a contract.
3. Code mode and `Eval` wrap tool results in another layer that also truncates, so the same result
   passes through N+1 independent limits with N+1 notices — and the innermost one may already
   have discarded what the outer consumer needed.

The sandbox boundary (0006) sharpens the requirement: a misused `Read` on the untrusted side can
stream 2 GB back, and nothing on that side can be trusted to stop it.

## Decision

- The runtime MUST retain the complete output of a call as an artifact and derive bounded
  projections for the model, the transcript, and remote clients. Consumers receive a projection
  plus an address; nobody receives the full result inline by default.
- The default limit MUST belong to the library. Tool authors NEVER implement truncation. A tool
  that emits more than the inline threshold is spilled by the call-outcome path, not by the tool.
- An explicit `notrunc` escape hatch MAY exist for a caller that needs the whole result inline. It
  is a caller decision recorded on the call, not a tool-level default.
- Truncation and other harness notices MUST be structured (`<diag severity="…">` in the call element,
  0008) and NEVER interpolated into the result body.
- Bounding MUST happen exactly once per result. Code mode, `Eval`, and any other wrapper consume the
  already-bounded projection and its artifact address; stacking a second limit is prohibited.
- Bounding MUST be enforced on the host side of the trust boundary, before the untrusted stream can
  exhaust host memory or context.

## Consequences

- Every tool gets correct, uniform truncation for free; the model sees one notice format.
- Full output is never lost: `Read` of the artifact address recovers it (0027), for the agent, the
  user, or a test.
- Prohibited: `maxBytes`-style parameters implemented per tool; string-slicing in tool bodies;
  prose notices; wrappers that re-truncate.
- Cost accepted: artifact storage and a content-addressed blob path become mandatory infrastructure
  for even the simplest deployment.

## Status in omp

**Implemented.** Primary implementation: `crates/agent/src/dispatch.rs`. Central output and line bounds spill full results to `omp_journal::blob`. `notrunc` is a caller request for complete inline output up to a fixed 8 MiB host security ceiling; it never disables host memory bounds, and larger results carry the same typed artifact-backed projection receipt as ordinary bounded calls. Source-backed projections carry typed byte spans through `omp-tool`; after the single central bound, the dispatcher returns only fully retained source lines in a `VisibilityReceipt`. `grep` stages revision-pinned bytes with the document authority before dispatch but authorizes lines only from that receipt, so it has no local byte heuristic or truncation footer. `crates/envd/src/exec.rs` enforces 64 KiB ordinary and 8 MiB complete-request limits before shell bytes enter the bounded event channel, while staging the complete byte stream directly into the environment CAS. Native-device and process-worker verdicts retain the full structured outcome in that CAS, bound model-facing parts at the environment boundary, and publish typed `OutputProjection` facts over `env/v1`. The driver verifies the request, byte counts, ceiling, and artifact identity before adopting a verdict. Detached-job settlement copies a runtime-spill artifact into the session CAS before the journal patch names it, so restart reconciliation cannot publish a dangling reference.

## References

- The Harness Playbook, "The runtime" — "Limits are part of the primitive", "Bound output once"
- 0002 (helpers become mandatory), 0006 (bounded streams across the trust boundary), 0008 (`<diag>`
  channel), 0027 (`Read` materializes `artifact://`), 0025 (Code mode / `Eval` as consumers)
- `crates/tool/src/lib.rs` (`CallOutcomeSpill`, `ThresholdWriter`, `Spilled`),
  `crates/agent/src/subagent.rs` (`SubagentDisposition`)
