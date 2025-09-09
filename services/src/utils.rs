//! Utilities module

use std::{env, path::PathBuf, process::Command};

#[derive(Clone)]
/// Where the executable is launched from
pub enum ExecutableLocation {
    /// Launch the executable at this exact filesystem path.
    ///
    /// The path is used verbatim—no `PATH` lookup is performed.
    Specific(PathBuf),

    /// Launch a program by name, resolved via the system `PATH`.
    ///
    /// This is equivalent to running the command name in a shell, without shell
    /// expansion. The binary must be discoverable in one of the `PATH` entries.
    Global(String),
}

impl ExecutableLocation {
    /// Construct a [`ExecutableLocation::Global`] from a program name.
    ///
    /// This is a convenience helper for creating `Global` locations.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::ExecutableLocation;
    /// let cargo = ExecutableLocation::by_name("cargo");
    /// ```
    pub fn by_name(name: &str) -> Self {
        Self::Global(name.to_string())
    }

    /// Build a [`std::process::Command`] targeting this executable.
    ///
    /// The returned command has its program set but no arguments or environment
    /// configured yet—you can add those before spawning.
    ///
    /// This function does not perform any I/O or validation; errors (e.g., the
    /// program not being found) will surface when you call methods like
    /// [`std::process::Command::status`], [`std::process::Command::spawn`], etc.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::PathBuf;
    /// # use std::process::Command;
    /// # use your_crate::ExecutableLocation;
    /// let loc = ExecutableLocation::Specific(PathBuf::from("/bin/echo"));
    /// let mut cmd = loc.command();
    /// cmd.arg("hi");
    /// // cmd.output().unwrap();
    /// ```
    pub(crate) fn command(&self) -> Command {
        match &self {
            Self::Specific(pathbuf) => Command::new(pathbuf),
            Self::Global(name) => Command::new(name),
        }
    }
}

impl std::fmt::Debug for ExecutableLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let actual_string = match self {
            ExecutableLocation::Specific(path_buf) => format!("Specific binary at {path_buf:?}"),
            ExecutableLocation::Global(name) => format!("Global binary named {name}"),
        };
        write!(f, "{actual_string}")?;
        Ok(())
    }
}

/// Returns path to cargo manifest directory (project root)
pub(crate) fn cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo manifest to resolve to pathbuf"))
}

/// Returns a path to the chain cache directory
pub fn chain_cache_dir() -> PathBuf {
    cargo_manifest_dir().join("chain_cache")
}
