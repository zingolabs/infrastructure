//! `zcash-local-net` — the harness's artifact pre-check CLI.
//!
//! Test suites that consume `zcash_local_net` describe where their
//! artifacts (zebrad, zainod, zcash-devtool) come from in a manifest
//! file; this CLI validates that manifest against the current
//! environment *before* the test runner starts, so a missing image or
//! binary is one clear report instead of N launch failures:
//!
//! ```sh
//! zcash-local-net preflight --manifest ci-artifacts.json \
//!   && ZCASH_LOCAL_NET_MANIFEST=ci-artifacts.json cargo nextest run
//! ```
//!
//! Argument parsing is hand-rolled: two subcommands and one flag do
//! not justify a CLI-parser dependency in the library crate.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zcash_local_net::container::manifest::{
    ArtifactManifest, DEFAULT_MANIFEST_FILENAME, MANIFEST_ENV_VAR,
};

const USAGE: &str = "\
zcash-local-net — artifact pre-checks for the zcash_local_net harness

USAGE:
    zcash-local-net preflight [--manifest <PATH>]
    zcash-local-net template
    zcash-local-net help

COMMANDS:
    preflight   Validate the artifact manifest against this machine:
                container runtime present and reachable, images
                present (pulled per their pull policy), host binaries
                resolvable and executable. Exits 0 when every check
                passes, 1 otherwise.
    template    Print an example manifest (JSON) to stdout.
    help        Print this message.

MANIFEST RESOLUTION (for `preflight`):
    1. --manifest <PATH>
    2. $ZCASH_LOCAL_NET_MANIFEST
    3. ./zcash-local-net.json, if it exists
    4. none — the built-in default (every artifact resolved as a host
       process via TEST_BINARIES_DIR / PATH)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preflight") => preflight(&args[1..]),
        Some("template") => {
            print!("{}", ArtifactManifest::template_json());
            ExitCode::SUCCESS
        }
        Some("help") | Some("--help") | Some("-h") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown command {other:?}\n\n{USAGE}");
            ExitCode::from(2)
        }
        None => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn preflight(args: &[String]) -> ExitCode {
    let manifest_path = match parse_manifest_flag(args) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    let (manifest, provenance) = match resolve_manifest(manifest_path) {
        Ok(resolved) => resolved,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("manifest: {provenance}");

    // The library is async throughout; the CLI hosts a minimal
    // current-thread runtime for the one call.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("the preflight runtime should build");
    let report = runtime.block_on(manifest.preflight());
    print!("{report}");
    if report.passed() {
        println!("preflight passed");
        ExitCode::SUCCESS
    } else {
        println!("preflight FAILED");
        ExitCode::FAILURE
    }
}

/// Parse `[--manifest <PATH>]`, rejecting anything else.
fn parse_manifest_flag(args: &[String]) -> Result<Option<PathBuf>, String> {
    match args {
        [] => Ok(None),
        [flag, path] if flag == "--manifest" => Ok(Some(PathBuf::from(path))),
        [flag] if flag == "--manifest" => Err("--manifest requires a path".to_string()),
        other => Err(format!("unexpected arguments: {other:?}")),
    }
}

/// Apply the documented manifest-resolution order, reporting which
/// source won so the user knows what was actually checked.
fn resolve_manifest(
    explicit: Option<PathBuf>,
) -> Result<(ArtifactManifest, String), Box<dyn std::error::Error>> {
    if let Some(path) = explicit {
        let manifest = ArtifactManifest::load(&path)?;
        return Ok((manifest, format!("{} (--manifest)", path.display())));
    }
    if let Some(path) = std::env::var_os(MANIFEST_ENV_VAR) {
        let path = PathBuf::from(path);
        let manifest = ArtifactManifest::load(&path)?;
        return Ok((
            manifest,
            format!("{} (${MANIFEST_ENV_VAR})", path.display()),
        ));
    }
    let cwd_manifest = Path::new(DEFAULT_MANIFEST_FILENAME);
    if cwd_manifest.exists() {
        let manifest = ArtifactManifest::load(cwd_manifest)?;
        return Ok((
            manifest,
            format!("./{DEFAULT_MANIFEST_FILENAME} (working directory)"),
        ));
    }
    Ok((
        ArtifactManifest::default(),
        "built-in default (host processes via TEST_BINARIES_DIR / PATH)".to_string(),
    ))
}
