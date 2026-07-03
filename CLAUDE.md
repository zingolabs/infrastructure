# `infras` AI Contributor Guidelines

This file collects conventions that AI contributors (and humans
reviewing AI-authored work) should apply to PRs in this repository.

## Tool selection

Always prefer Rust-native tools in domains where they are designed to
operate. Dependency and manifest changes go through `cargo add` /
`cargo remove` / `cargo update`. Code navigation and refactors go
through rust-analyzer. Verification goes through `cargo check` /
`cargo clippy` / `cargo fmt` / `cargo nextest`. Do not reach for
Python, sed, or regex sweeps over Rust source or `Cargo.toml` when a
Rust tool covers the job.

## Code-review checklist: bugs Rust won't catch

Memory-safety guarantees and the type system catch a lot, but the
following bug classes pass the borrow checker cleanly and still ship as
real vulnerabilities or correctness failures. Audit every PR touching
filesystem, parsing, RPC boundaries, or external-input handling against
this list. Distilled from
<https://corrode.dev/blog/bugs-rust-wont-catch/>.

### 1. TOCTOU on filesystem paths

Operating on the same `&Path` across multiple syscalls lets an attacker
swap a symlink between the check and the use, redirecting privileged
operations to unintended targets.

- **Look for**: any `&Path` variable consumed by two or more syscalls
  (e.g. `fs::metadata(p)` then `File::open(p)`, or `fs::create_dir(p)`
  then `fs::set_permissions(p, ..)`); `fs::remove_file` or
  `set_permissions` after creation.
- **Mitigate**: prefer `OpenOptions::create_new(true)` (refuses
  pre-existing or symlinked targets); anchor follow-up operations on
  the file descriptor returned by the open, not on the path; use the
  `*at` family of syscalls relative to an open directory handle.

### 2. Delayed permission setting

Two-step "create then chmod" leaves a window where the file/dir exists
with default (often world-readable) permissions before the restrictive
mode lands.

- **Look for**: `File::create` / `fs::create_dir(_all)` immediately
  followed by `fs::set_permissions` or `chmod`; tests/fixtures where
  permissions are tightened after construction.
- **Mitigate**: set the mode atomically at creation —
  `OpenOptions::mode(0o600).create_new(true).open(..)` and
  `DirBuilderExt::mode(..)`. If process-wide control is needed, set
  `umask` explicitly at startup.

### 3. Path equality compared as strings

`==` on `Path`/`PathBuf` is a *string* comparison. It does not resolve
`./`, `..`, symlinks, or case-folded equivalents — two distinct strings
can name the same inode.

- **Look for**: `path == Path::new("..")`, `path.starts_with("/safe")`,
  or any security/authorization decision made on a path string without
  prior resolution.
- **Mitigate**: `fs::canonicalize` both sides before comparing; for
  arbitrary or non-canonicalizable paths, compare `(dev, inode)` pairs
  via `fs::metadata`.

### 4. Silent UTF-8 corruption of binary streams

`String::from_utf8_lossy` silently rewrites invalid bytes as U+FFFD;
round-tripping binary data through `String` (or printing it via
`print!`/`Display`) corrupts content.

- **Look for**: `from_utf8_lossy` on file/network input that may not be
  text; `print!("{}", buf)` on `Vec<u8>` or other byte buffers;
  `Display`/`{}` formatting applied to wire/protocol bytes.
- **Mitigate**: stay in `Vec<u8>` / `&[u8]` end-to-end. Use
  `Write::write_all` for binary output. Convert to `String` only at
  trust-validated text boundaries.

### 5. Panics on untrusted input → DoS

`unwrap()`, `expect()`, slice indexing, integer arithmetic without
`checked_*`, and parsing helpers like `from_utf8().expect()` all panic
on adversarial input, taking the whole process down.

- **Look for**: `.unwrap()` / `.expect()` on data that crossed a
  trust boundary (process stdout/stderr that the harness scrapes,
  child-process exit-code parses, deserialised RPC responses); `[i]`
  and `[..n]` slicing on input-derived indices; `as u32` / `+` / `*`
  on attacker-controlled sizes.
- **Mitigate**: propagate with `?`, return a typed error, or handle
  the `None`/`Err` case explicitly. Use `.get(i)`, `.checked_add`,
  `usize::try_from`, etc. Turn on the relevant clippy lints
  (`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`,
  `arithmetic_side_effects`) at module scope where feasible.

### 6. Discarded `Result` values

Throwing a `Result` away with `.ok()`, `let _ = ...`, or
`unwrap_or_default()` collapses the error into success and leaves
downstream code operating on stale or invalid state.

- **Look for**: `.ok();` at end of statement, `let _ = some_call();`
  without a justifying comment, `unwrap_or_default()` where default is
  semantically meaningful, loops that return only the *last* error
  encountered.
- **Mitigate**: propagate or aggregate. In a loop that may have
  multiple failures, track the worst exit code / first error and
  return at the end. Any deliberate discard requires an inline comment
  explaining why losing the error is safe.

### 7. Behaviour drift from a contract this code mirrors

Where this harness wraps an external tool (`zcashd` / `zebrad` /
`lightwalletd` CLI invocations, their RPC endpoints, exit codes,
stdout/stderr indicator strings), callers script against the original
tool's quirks. Subtle semantic differences — flag interpretation, exit
code, error message text, the exact bytes written to a log file —
break harness consumers silently.

- **Look for**: new validator/indexer wrappers whose behaviour is
  derived from reading the upstream tool's docs rather than from a
  passing test against the actual binary; collision-signature lists
  whose entries weren't cross-checked against the binary's real bind
  failure output; novel `LaunchError` variants / exit-code mappings
  added without a regression test that exercises the failure mode.
- **Mitigate**: pin the contract with a test that runs against the
  upstream binary (or a recorded golden response) — not with
  hand-written assertions about what we think the contract is. The
  `mod launch_recovers_from_rpc_port_collision` shape in
  `zcash_local_net::tests::integration` is the canonical example:
  every collision-signature list lives next to a test that *actually
  collides the port and asserts the signature matched*.

### 8. Inputs resolved on the wrong side of a trust boundary

When user-controlled inputs are looked up via dynamically-loaded
machinery (NSS modules, plugin systems, dlopen-style backends) *after*
the process enters a more-trusted or more-restricted context, the
attacker controls the lookup path.

- **Look for**: any `chroot`, `setuid`, namespace transition, or
  switch into a sandboxed context that is followed by a name
  resolution call (user/group lookup, hostname resolution,
  configuration load, plugin discovery); `dlopen` or dynamic backend
  selection performed late.
- **Mitigate**: resolve all names and load all dynamic backends
  *before* the boundary transition. Static linking does not help if
  the resolver itself is dynamic (e.g. glibc NSS).

### How to apply this checklist

- On any PR that adds or modifies code touching the filesystem, byte
  streams, parsing of external input, RPC handlers, or process
  privilege/sandbox transitions, walk this list explicitly. The cost
  is a minute per PR; the cost of one of these landing is much higher.
- When you spot a category-(N) site already in tree without a fix,
  open a tracking issue rather than silently shipping the audit
  fix — the bug is older than your PR and may have caller-side
  expectations that need to change in lockstep.
