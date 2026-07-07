#![forbid(unsafe_code)]
//! Developer and CI tooling for this repository.
//!
//! Checked-in scripting that collaborators or CI run lives here, in
//! Rust, per the contributor guidelines. Current subcommands:
//!
//! - `trailing-whitespace (fix | reject)` — the port of the retired
//!   `utils/trailing-whitespace.sh`, run by the Trailing Whitespace CI
//!   workflow in `reject` mode.
//! - `check-external-types` — the port of the retired justfile: runs
//!   `cargo check-external-types` under the pinned nightly for every
//!   crate with a public-API allowlist. Run by the External Types CI
//!   job.
//! - `generate-chain-caches` — builds the large zebrad chain cache
//!   that cache-dependent tests consume.

use std::io::Read as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// File extensions the whitespace checker treats as well-known text,
/// mirroring the shell script it replaced.
const TEXT_EXTENSIONS: [&str; 4] = ["rs", "md", "toml", "yaml"];

/// Directory names pruned from the walk, mirroring the shell script.
const PRUNED_DIRS: [&str; 2] = [".git", "target"];

/// The nightly toolchain cargo-check-external-types requires. Pinned
/// to match cargo-check-external-types 0.4.0 (rustdoc JSON format 56;
/// 0.5.0 requires format 57 — bump both together, and keep the
/// External Types job in `.github/workflows/ci-pr.yaml` on the same
/// pin: it installs this toolchain and that tool version by name.
const PINNED_NIGHTLY: &str = "nightly-2025-10-18";

/// The manifests of every crate whose public API carries an
/// external-types allowlist.
const EXTERNAL_TYPES_MANIFESTS: [&str; 2] = [
    "zcash_local_net/Cargo.toml",
    "zingo_test_vectors/Cargo.toml",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["trailing-whitespace", "fix"] => trailing_whitespace(Mode::Fix),
        ["trailing-whitespace", "reject"] => trailing_whitespace(Mode::Reject),
        ["check-external-types"] => check_external_types(),
        ["generate-chain-caches"] => generate_chain_caches(),
        _ => {
            eprintln!(
                "usage: workbench ( trailing-whitespace ( fix | reject ) \
                 | check-external-types | generate-chain-caches )"
            );
            return ExitCode::FAILURE;
        }
    };
    match outcome {
        Ok(exit) => exit,
        Err(error) => {
            eprintln!("workbench: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Run `cargo check-external-types` under [`PINNED_NIGHTLY`] for every
/// manifest in [`EXTERNAL_TYPES_MANIFESTS`], streaming each tool's own
/// output. Every manifest is checked even after a failure, so one run
/// reports every offending crate; the exit code is a failure if any
/// check failed.
fn check_external_types() -> std::io::Result<ExitCode> {
    let root = repository_root()?;
    let mut failures = Vec::new();
    for manifest in EXTERNAL_TYPES_MANIFESTS {
        let status = std::process::Command::new("cargo")
            .arg(format!("+{PINNED_NIGHTLY}"))
            .args(["check-external-types", "--manifest-path", manifest])
            .current_dir(&root)
            .status()?;
        if !status.success() {
            failures.push(manifest);
        }
    }
    if failures.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("check-external-types failed for: {}", failures.join(", "));
        Ok(ExitCode::FAILURE)
    }
}

/// Build the large zebrad chain cache that cache-dependent tests
/// consume, by running the ignored generator test.
fn generate_chain_caches() -> std::io::Result<ExitCode> {
    let root = repository_root()?;
    let status = std::process::Command::new("cargo")
        .args([
            "nextest",
            "run",
            "generate_zebrad_large_chain_cache",
            "--run-ignored",
            "ignored-only",
        ])
        .current_dir(&root)
        .status()?;
    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

enum Mode {
    /// Strip trailing spaces in place.
    Fix,
    /// Report offending lines and fail if any exist.
    Reject,
}

/// Walk the repository's well-known text files and either strip or
/// reject trailing space characters. Tabs are deliberately not
/// trailing whitespace here: the shell script this replaces matched
/// only spaces (`' +$'` / `s/ *$//`), and the port preserves that
/// contract. Files are processed as bytes end to end, so non-UTF-8
/// content passes through untouched.
fn trailing_whitespace(mode: Mode) -> std::io::Result<ExitCode> {
    let root = repository_root()?;
    let mut files = Vec::new();
    collect_text_files(&root, &mut files)?;

    let mut offending_lines: u64 = 0;
    let mut fixed_files: u64 = 0;
    for path in &files {
        let mut content = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut content)?;
        match mode {
            Mode::Reject => {
                for (line_number, line) in lines_with_trailing_spaces(&content) {
                    let display = path.strip_prefix(&root).unwrap_or(path);
                    println!(
                        "{}:{line_number}: {}",
                        display.display(),
                        String::from_utf8_lossy(line).trim_end()
                    );
                    offending_lines += 1;
                }
            }
            Mode::Fix => {
                let stripped = strip_trailing_spaces(&content);
                if stripped != content {
                    std::fs::File::create(path)?.write_all(&stripped)?;
                    fixed_files += 1;
                }
            }
        }
    }

    match mode {
        Mode::Reject if offending_lines == 0 => {
            println!("No trailing whitespace detected.");
            Ok(ExitCode::SUCCESS)
        }
        Mode::Reject => {
            println!("\nRejecting {offending_lines} line(s) of trailing whitespace above.");
            Ok(ExitCode::FAILURE)
        }
        Mode::Fix => {
            println!("Stripped trailing whitespace from {fixed_files} file(s).");
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Ascend from the current directory to the first ancestor containing
/// a `.git` entry. Anchoring the walk at the repository root keeps the
/// tool's behavior independent of the invocation directory, as the
/// shell script's `git rev-parse --show-toplevel` did.
fn repository_root() -> std::io::Result<PathBuf> {
    let start = std::env::current_dir()?;
    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }
    }
    Err(std::io::Error::other(format!(
        "no .git directory found in any ancestor of {}",
        start.display()
    )))
}

/// Recursively collect the well-known text files under `dir`, pruning
/// [`PRUNED_DIRS`] and ignoring symlinks (the shell script's
/// `find -type f` did not follow them either).
fn collect_text_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            let name = entry.file_name();
            if PRUNED_DIRS.iter().any(|pruned| name == *pruned) {
                continue;
            }
            collect_text_files(&path, files)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|ext| TEXT_EXTENSIONS.iter().any(|known| ext == *known))
        {
            files.push(path);
        }
    }
    Ok(())
}

/// The 1-based line numbers and contents of lines ending in one or
/// more space characters. A line's terminator is `\n`; a `\r` before
/// it counts as content, so CRLF lines never match — exactly the
/// behavior of the shell script's `grep -E ' +$'`.
fn lines_with_trailing_spaces(content: &[u8]) -> Vec<(usize, &[u8])> {
    content
        .split_inclusive(|&byte| byte == b'\n')
        .enumerate()
        .filter_map(|(index, line)| {
            let body = line.strip_suffix(b"\n").unwrap_or(line);
            body.ends_with(b" ").then_some((index + 1, body))
        })
        .collect()
}

/// `content` with every line's trailing run of space characters
/// removed, line terminators and all other bytes preserved.
fn strip_trailing_spaces(content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len());
    for line in content.split_inclusive(|&byte| byte == b'\n') {
        let (body, terminator) = match line.strip_suffix(b"\n") {
            Some(body) => (body, &b"\n"[..]),
            None => (line, &b""[..]),
        };
        let end = body
            .iter()
            .rposition(|&byte| byte != b' ')
            .map_or(0, |position| position + 1);
        out.extend_from_slice(&body[..end]);
        out.extend_from_slice(terminator);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_spaces_are_stripped_and_terminators_preserved() {
        let input = b"clean line\ndirty line   \nlast line no newline  ";
        assert_eq!(
            strip_trailing_spaces(input),
            b"clean line\ndirty line\nlast line no newline"
        );
    }

    #[test]
    fn tabs_and_crlf_are_not_trailing_whitespace() {
        // The shell script matched only spaces; tabs and the \r of a
        // CRLF terminator count as content and stay untouched.
        let input = b"tab line\t\ncrlf line \r\n";
        assert_eq!(strip_trailing_spaces(input), input.as_slice());
        assert!(lines_with_trailing_spaces(input).is_empty());
    }

    #[test]
    fn non_utf8_bytes_pass_through_untouched() {
        let input = b"\xff\xfe binary-ish \xff\n";
        let stripped = strip_trailing_spaces(input);
        assert_eq!(stripped, b"\xff\xfe binary-ish \xff\n");
    }

    #[test]
    fn offending_lines_report_one_based_numbers() {
        let input = b"ok\nbad \nok\nalso bad  \n";
        let found = lines_with_trailing_spaces(input);
        assert_eq!(
            found,
            vec![(2, b"bad ".as_slice()), (4, b"also bad  ".as_slice())]
        );
    }

    #[test]
    fn walk_collects_known_extensions_and_prunes_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.rs"), "x").unwrap();
        std::fs::write(root.join("b.txt"), "x").unwrap();
        std::fs::create_dir(root.join("target")).unwrap();
        std::fs::write(root.join("target").join("c.rs"), "x").unwrap();
        std::fs::create_dir(root.join("nested")).unwrap();
        std::fs::write(root.join("nested").join("d.yaml"), "x").unwrap();

        let mut files = Vec::new();
        collect_text_files(root, &mut files).unwrap();
        let mut names: Vec<String> = files
            .iter()
            .map(|path| {
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(names, ["a.rs", "nested/d.yaml"]);
    }
}
