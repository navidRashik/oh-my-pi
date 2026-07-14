Coordinate with known peer agents during active multi-agent work.

# Scope
You MUST use IRC only when a `task` result names a peer, an IRC message arrives, or the user explicitly requests peer coordination.
- Your IRC id: `{{selfId}}`. NEVER send messages to yourself.
- Without a coordination trigger, you MUST use task-relevant tools; NEVER call `list`, `inbox`, or `wait` speculatively.

# Addressing and Discovery
The main agent id is `Main`; subagents inherit their task id (e.g., `AuthLoader`). Address peers by exact ids from task results or incoming messages; NEVER invent names. During active coordination, you MAY use `op: "list"` to recover a roster after ids become unavailable. NEVER use `list` to check whether peers exist.

# Messaging Rules
Use `op: "send"` to deliver a message to a specific peer or broadcast to `"all"`.
- **Fire and forget:** Sending NEVER blocks. You get delivery receipts immediately (`delivered` or `failed`). Do not wait around—send your message and keep working. If a receipt says `failed`, the peer is gone; do not retry.
- **Waking peers:** Sending a message to an `idle` or `parked` agent automatically wakes them up.
- **Answering:** When replying to a question, use `op: "send"`, lead directly with your answer (NEVER quote the original message), and set `replyTo` so the recipient can correlate it.
- **Format:** Messages MUST be plain prose. NEVER send JSON status objects. Keep it terse and share paths via `local://` or `artifact://` URLs, not pasted blobs.

# Waiting and Inboxes
Messages only arrive when the peer actively sends one—do not interrogate a peer for status.
- If you are completely blocked and MUST wait for an answer, use `op: "wait"` (or `await: true` on a send). The wait returns when a matching message arrives, the timeout elapses, or any IRC / steering message interrupts the wait. Parent-agent IRC interrupts with steering-level priority.
- No need to alternate `irc wait`, `irc inbox`, and `job poll`: waits surface cross-channel interrupts promptly. The next turn includes the interrupt reason and message.
- After an IRC notification, you MAY use `op: "inbox"` to drain queued messages without blocking.

# When to Coordinate
During active coordination, message known peers instead of guessing, duplicating work, or spying.
- If a known peer has context needed for an unexpected state or out-of-scope decision, you SHOULD message that peer.
- Before editing overlapping work, you MUST message the known peer touching it.
- NEVER use shell tools, grep, or read other sessions' files to figure out what a peer is doing. Message them directly.
- NEVER use IRC for something a tool can answer (e.g., grepping codebase, running a build).
