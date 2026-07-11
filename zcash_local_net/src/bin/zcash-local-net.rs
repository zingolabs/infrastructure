//! `zcash-local-net` — the harness's artifact pre-check CLI.
//!
//! Test suites that consume `zcash_local_net` describe where their
//! artifacts (zebrad, zainod, zcash-devtool) come from in a manifest
//! file; this CLI validates that manifest against the current
//! environment *before* the test runner starts, so a missing image or
//! binary is one clear report instead of N launch failures:
//!
//! ```sh
//! zcash-local-net preflight --manifest ci-artifacts.toml \
//!   && ZCASH_LOCAL_NET_MANIFEST=ci-artifacts.toml cargo nextest run
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
    zcash-local-net update [--manifest <PATH>] [validator|indexer|wallet ...]
    zcash-local-net template
    zcash-local-net help

COMMANDS:
    preflight   Validate the artifact manifest against this machine:
                container runtime present and reachable, images
                present (pulled per their pull policy), host binaries
                resolvable and executable. Exits 0 when every check
                passes, 1 otherwise.
    update      Explicitly bump the manifest's image pins: resolve each
                artifact's tracked reference (its `track` field, or the
                tag its `image` was pinned from) against the registry
                and rewrite `image` to the digest-pinned result
                (`repo:tag@sha256:…`). Pins never move at run time —
                this command is the only thing that moves them. Name
                artifacts to bump a subset. The file is rewritten in
                canonical formatting; review the diff and commit it.
    template    Print an example manifest (TOML) to stdout.
    help        Print this message.

MANIFEST RESOLUTION:
    1. --manifest <PATH>
    2. $ZCASH_LOCAL_NET_MANIFEST
    3. ./zcash-local-net.toml, if it exists
    4. none — for `preflight`, the built-in default (every artifact
       resolved as a host process via TEST_BINARIES_DIR / PATH);
       `update` needs a file to rewrite and exits with an error.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("preflight") => preflight(&args[1..]),
        Some("update") => update(&args[1..]),
        Some("template") => {
            print!("{}", ArtifactManifest::template_toml());
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

fn update(args: &[String]) -> ExitCode {
    // `update` shares the --manifest flag but additionally accepts
    // artifact names.
    let mut manifest_flag: Option<PathBuf> = None;
    let mut artifacts: Vec<&str> = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--manifest" {
            match rest.next() {
                Some(path) => manifest_flag = Some(PathBuf::from(path)),
                None => {
                    eprintln!("--manifest requires a path\n\n{USAGE}");
                    return ExitCode::from(2);
                }
            }
        } else {
            artifacts.push(arg.as_str());
        }
    }

    let (path, provenance) = match resolve_manifest_path(manifest_flag) {
        Some(resolved) => resolved,
        None => {
            eprintln!(
                "error: no manifest file to update — pass --manifest, set \
                 ${MANIFEST_ENV_VAR}, or create ./{DEFAULT_MANIFEST_FILENAME}"
            );
            return ExitCode::FAILURE;
        }
    };
    println!("manifest: {provenance}");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("the update runtime should build");
    match runtime.block_on(zcash_local_net::container::update::update_manifest_file(
        &path, &artifacts,
    )) {
        Ok(bumps) if bumps.is_empty() => {
            println!("no artifact has a floating reference to follow; nothing to update");
            ExitCode::SUCCESS
        }
        Ok(bumps) => {
            for bump in &bumps {
                if bump.changed() {
                    let previous = if bump.previous.is_empty() {
                        "<no pin yet>"
                    } else {
                        &bump.previous
                    };
                    println!(
                        "{}: {} resolved; {previous} -> {}",
                        bump.artifact, bump.tracked, bump.pinned
                    );
                } else {
                    println!("{}: {} already current", bump.artifact, bump.tracked);
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
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

/// The documented resolution order, for commands that need the file
/// itself rather than a loaded manifest. `None` when nothing resolves.
fn resolve_manifest_path(explicit: Option<PathBuf>) -> Option<(PathBuf, String)> {
    if let Some(path) = explicit {
        let provenance = format!("{} (--manifest)", path.display());
        return Some((path, provenance));
    }
    if let Some(path) = std::env::var_os(MANIFEST_ENV_VAR) {
        let path = PathBuf::from(path);
        let provenance = format!("{} (${MANIFEST_ENV_VAR})", path.display());
        return Some((path, provenance));
    }
    let cwd_manifest = Path::new(DEFAULT_MANIFEST_FILENAME);
    cwd_manifest.exists().then(|| {
        (
            cwd_manifest.to_path_buf(),
            format!("./{DEFAULT_MANIFEST_FILENAME} (working directory)"),
        )
    })
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
