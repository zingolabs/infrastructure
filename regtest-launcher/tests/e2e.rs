use regex::Regex;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::timeout,
};

#[cfg(unix)]
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};

/// Compare `actual` against the golden file at `tests/golden/<name>.txt`,
/// ignoring trailing-newline differences. Set `UPDATE_GOLDEN=1` to rewrite
/// the golden file from the current output instead of asserting.
fn assert_matches_golden(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.txt"));

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).expect("write golden file");
        return;
    }

    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("golden file {} unreadable: {e}", path.display()));
    assert_eq!(
        actual.trim_end_matches('\n'),
        expected.trim_end_matches('\n'),
        "output diverged from golden file {} — rerun with UPDATE_GOLDEN=1 to bless",
        path.display()
    );
}

fn normalized_dynamic_values(s: &str) -> String {
    // Strip ANSI escape sequences
    let regex_ansi = Regex::new(r"\x1b\[[0-9;]*m").unwrap();

    let regex_localhost_port = Regex::new(r"(127\.0\.0\.1:)\d+").unwrap();

    let regex_mined_up_to = Regex::new(r"(Mined up to chain height\s+)\d+").unwrap();
    let regex_height_inline = Regex::new(r"(height=)\d+").unwrap();

    let regex_hash64 = Regex::new(r"\b[0-9a-f]{64}\b").unwrap();

    let text = regex_ansi.replace_all(s, "");
    let text = regex_localhost_port.replace_all(&text, "$1<PORT>");

    let text = regex_mined_up_to.replace_all(&text, "$1<HEIGHT>");
    let text = regex_height_inline.replace_all(&text, "$1<HEIGHT>");

    let text = regex_hash64.replace_all(&text, "<HASH>");

    text.to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mines_once_then_exits_on_ctrlc() -> anyhow::Result<()> {
    // Cargo sets CARGO_BIN_EXE_<name> for integration tests of crates
    // with a bin target — no helper crate needed to locate the binary.
    let executable_path = env!("CARGO_BIN_EXE_regtest-launcher");

    let mut child_process = Command::new(executable_path)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let pid = child_process.id().expect("child pid");

    let mut err_lines = BufReader::new(child_process.stderr.take().expect("stderr")).lines();
    let err_task = tokio::spawn(async move {
        let mut err = String::new();
        while let Some(line) = err_lines.next_line().await? {
            err.push_str(&line);
            err.push('\n');
        }
        Ok::<String, anyhow::Error>(err)
    });

    let mut out_lines = BufReader::new(child_process.stdout.take().expect("stdout")).lines();
    let mut captured_out = String::new();

    let (saw_indexer, saw_mined) = timeout(Duration::from_secs(45), async {
        let mut saw_indexer = false;
        let mut saw_mined = false;

        while let Some(line) = out_lines.next_line().await? {
            captured_out.push_str(&line);
            captured_out.push('\n');

            if line.contains("Indexer running at: 127.0.0.1:") {
                saw_indexer = true;
            }

            // mined new_tip=<hash> height=<num>
            if line.contains("mined new_tip=") {
                saw_mined = true;
                break;
            }
        }

        Ok::<_, anyhow::Error>((saw_indexer, saw_mined))
    })
    .await??;

    assert!(
        saw_indexer,
        "never saw indexer startup line.\nstdout:\n{captured_out}"
    );
    assert!(saw_mined, "never saw mined line.\nstdout:\n{captured_out}");

    #[cfg(unix)]
    kill(Pid::from_raw(pid as i32), Signal::SIGINT)?;

    #[cfg(not(unix))]
    child_process.kill().await.ok();

    let status = timeout(Duration::from_secs(30), child_process.wait()).await??;
    let captured_err = err_task.await??;

    assert!(status.success(), "process failed.\nstderr:\n{captured_err}");
    assert!(
        !captured_err.to_lowercase().contains("panic"),
        "panic seen.\nstderr:\n{captured_err}"
    );

    // Golden snapshot
    let normalized_stdout = normalized_dynamic_values(&captured_out);
    assert_matches_golden("regtest_first_mine", &normalized_stdout);

    Ok(())
}
