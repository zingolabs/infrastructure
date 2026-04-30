//! FD-anchored, no-symlink-following recursive directory copy.
//!
//! Closes the TOCTOU window between path resolution and writing that
//! opens when `Path::exists()` (or a `cp -r` subprocess) re-resolves
//! a path through attacker-controlled symlink ancestors. See issue
//! #256 for the audit-test sites that adopt this helper.
//!
//! Public API:
//!
//! - [`open_dir_no_symlinks`] -- opens an absolute path as a
//!   directory FD, walking each component with
//!   `openat(O_NOFOLLOW|O_DIRECTORY)` from `/`. Refuses any symlink
//!   ancestor; refuses `..` components and non-absolute paths.
//!
//! - [`safe_copy_into_new`] -- copies `trusted_src` into `dst`
//!   (which must not exist). The path to `dst.parent()` is walked
//!   from `/` rejecting symlink ancestors, then the leaf is created
//!   atomically via `mkdirat`. Used by `Validator::cache_chain`
//!   (issue #256, A2).
//!
//! - [`safe_copy_into_existing`] -- copies `src` into `trusted_dst`
//!   as `trusted_dst/<src.basename>/`. The path to `src` is walked
//!   from `/` rejecting symlink ancestors and `src.basename` itself
//!   is opened with `O_NOFOLLOW`. Used by
//!   `Zebrad::load_chain`/`Zcashd::load_chain` (issue #256, A3/A4).
//!
//! Internally the recursive copy uses only `*at` syscalls relative
//! to FDs we hold open, so no path is re-resolved by the kernel
//! between steps.

use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};
use std::path::{Component, Path};

use nix::dir::Dir;
use nix::fcntl::{AtFlags, OFlag};
use nix::sys::stat::{Mode, SFlag};
use nix::NixPath;

/// `nix::fcntl::openat` returns `RawFd` in 0.29 and takes `Option<RawFd>`
/// for the directory FD. Wrap it once so call sites stay in
/// `BorrowedFd`/`OwnedFd` and the `unsafe { from_raw_fd }` lives in
/// exactly one place.
fn openat_owned<P>(
    dirfd: BorrowedFd<'_>,
    path: &P,
    flags: OFlag,
    mode: Mode,
) -> io::Result<OwnedFd>
where
    P: ?Sized + NixPath,
{
    let raw = nix::fcntl::openat(Some(dirfd.as_raw_fd()), path, flags, mode)?;
    // SAFETY: openat returned a fresh FD that no one else holds.
    Ok(unsafe { OwnedFd::from_raw_fd(raw) })
}

/// Open `path` as a directory FD, walking from `/` with
/// `openat(O_NOFOLLOW|O_DIRECTORY)` at each component. Rejects any
/// symlink ancestor (`ELOOP` from the kernel, surfaced as
/// `io::Error`). Also rejects `..` components and non-absolute paths.
pub fn open_dir_no_symlinks(path: &Path) -> io::Result<OwnedFd> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must be absolute",
        ));
    }

    let mut current: OwnedFd = File::open("/")?.into();

    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => continue,
            Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    ".. components are not allowed",
                ));
            }
            Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "windows path prefixes are not allowed",
                ));
            }
            Component::Normal(name) => {
                current = openat_owned(
                    current.as_fd(),
                    name,
                    OFlag::O_RDONLY
                        | OFlag::O_DIRECTORY
                        | OFlag::O_NOFOLLOW
                        | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )?;
            }
        }
    }
    Ok(current)
}

/// Copy `trusted_src` (an existing directory) to `dst` (must not yet
/// exist). `dst.parent()` is walked from `/` rejecting symlink
/// ancestors; the leaf is created atomically via `mkdirat`; contents
/// are copied via `*at` syscalls relative to held FDs.
pub fn safe_copy_into_new(trusted_src: &Path, dst: &Path) -> io::Result<()> {
    let dst_parent = dst.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "dst must have a parent")
    })?;
    let dst_basename = dst.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "dst must have a basename")
    })?;

    let dst_parent_fd = open_dir_no_symlinks(dst_parent)?;

    nix::sys::stat::mkdirat(
        Some(dst_parent_fd.as_raw_fd()),
        dst_basename,
        Mode::S_IRWXU,
    )?;

    let dst_fd: OwnedFd = openat_owned(
        dst_parent_fd.as_fd(),
        dst_basename,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?;

    let src_fd: OwnedFd = File::open(trusted_src)?.into();

    copy_dir_contents(src_fd.as_fd(), dst_fd.as_fd())
}

/// Copy `src` (an existing directory) into `trusted_dst` (an
/// existing directory) as `trusted_dst/<src.basename>/`. `src`'s
/// parent is walked from `/` rejecting symlink ancestors, and
/// `src.basename` itself is opened with `O_NOFOLLOW` -- so neither
/// `src` nor any ancestor of `src` may be a symlink.
pub fn safe_copy_into_existing(src: &Path, trusted_dst: &Path) -> io::Result<()> {
    let src_parent = src.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "src must have a parent")
    })?;
    let src_basename = src.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "src must have a basename")
    })?;

    let src_parent_fd = open_dir_no_symlinks(src_parent)?;

    let src_fd: OwnedFd = openat_owned(
        src_parent_fd.as_fd(),
        src_basename,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?;

    let trusted_dst_fd: OwnedFd = File::open(trusted_dst)?.into();

    nix::sys::stat::mkdirat(
        Some(trusted_dst_fd.as_raw_fd()),
        src_basename,
        Mode::S_IRWXU,
    )?;

    let new_dst_fd: OwnedFd = openat_owned(
        trusted_dst_fd.as_fd(),
        src_basename,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?;

    copy_dir_contents(src_fd.as_fd(), new_dst_fd.as_fd())
}

fn copy_dir_contents(src_fd: BorrowedFd<'_>, dst_fd: BorrowedFd<'_>) -> io::Result<()> {
    // `Dir::from_fd` takes ownership of the FD it iterates with; dup
    // so the caller-owned src_fd remains usable for openat below.
    let src_dup = src_fd.try_clone_to_owned()?;
    let mut dir = Dir::from_fd(src_dup.into_raw_fd())?;

    for entry_result in dir.iter() {
        let entry = entry_result?;
        let name = entry.file_name();
        let name_bytes = name.to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }

        let stat = nix::sys::stat::fstatat(
            Some(src_fd.as_raw_fd()),
            name,
            AtFlags::AT_SYMLINK_NOFOLLOW,
        )?;

        let mode_bits = Mode::from_bits_truncate(stat.st_mode);
        let file_type = stat.st_mode & SFlag::S_IFMT.bits();

        if file_type == SFlag::S_IFDIR.bits() {
            nix::sys::stat::mkdirat(Some(dst_fd.as_raw_fd()), name, mode_bits)?;

            let new_src: OwnedFd = openat_owned(
                src_fd,
                name,
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )?;
            let new_dst: OwnedFd = openat_owned(
                dst_fd,
                name,
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )?;

            copy_dir_contents(new_src.as_fd(), new_dst.as_fd())?;
        } else if file_type == SFlag::S_IFREG.bits() {
            let src_file_fd: OwnedFd = openat_owned(
                src_fd,
                name,
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )?;
            let dst_file_fd: OwnedFd = openat_owned(
                dst_fd,
                name,
                OFlag::O_WRONLY
                    | OFlag::O_CREAT
                    | OFlag::O_EXCL
                    | OFlag::O_NOFOLLOW
                    | OFlag::O_CLOEXEC,
                mode_bits,
            )?;

            let mut src_file: File = src_file_fd.into();
            let mut dst_file: File = dst_file_fd.into();
            io::copy(&mut src_file, &mut dst_file)?;
        } else if file_type == SFlag::S_IFLNK.bits() {
            let target = nix::fcntl::readlinkat(Some(src_fd.as_raw_fd()), name)?;
            nix::unistd::symlinkat(
                target.as_os_str(),
                Some(dst_fd.as_raw_fd()),
                name,
            )?;
        } else {
            tracing::warn!(
                ?name,
                "skipping non-regular non-directory entry in safe_copy"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_dir_no_symlinks_rejects_relative_path() {
        let err = open_dir_no_symlinks(Path::new("relative")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn open_dir_no_symlinks_rejects_dotdot_components() {
        let err = open_dir_no_symlinks(Path::new("/tmp/..")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn open_dir_no_symlinks_rejects_symlink_ancestor() {
        let outer = tempfile::tempdir().unwrap();
        let real = outer.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = outer.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let through_link = link.join("");
        let err = open_dir_no_symlinks(&through_link).unwrap_err();
        // ELOOP becomes a FilesystemLoop / Other depending on rust
        // version; the important thing is the call does not succeed.
        assert!(
            err.raw_os_error() == Some(nix::libc::ELOOP)
                || err.raw_os_error() == Some(nix::libc::ENOTDIR),
            "expected ELOOP/ENOTDIR, got {err:?}"
        );
    }

    #[test]
    fn safe_copy_into_new_copies_nested_tree() {
        let src_holder = tempfile::tempdir().unwrap();
        std::fs::write(src_holder.path().join("a.txt"), b"contents A").unwrap();
        std::fs::create_dir(src_holder.path().join("sub")).unwrap();
        std::fs::write(src_holder.path().join("sub").join("b.txt"), b"B").unwrap();

        let dst_holder = tempfile::tempdir().unwrap();
        let dst = dst_holder.path().join("dst");

        safe_copy_into_new(src_holder.path(), &dst).unwrap();

        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"contents A");
        assert_eq!(std::fs::read(dst.join("sub").join("b.txt")).unwrap(), b"B");
    }

    #[test]
    fn safe_copy_into_new_refuses_existing_dst() {
        let src_holder = tempfile::tempdir().unwrap();
        std::fs::write(src_holder.path().join("a"), b"a").unwrap();
        let dst_holder = tempfile::tempdir().unwrap();
        let dst = dst_holder.path().join("dst");
        std::fs::create_dir(&dst).unwrap();

        let err = safe_copy_into_new(src_holder.path(), &dst).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(nix::libc::EEXIST));
    }

    #[test]
    fn safe_copy_into_new_refuses_parent_symlink() {
        let attacker = tempfile::tempdir().unwrap();
        let cache_holder = tempfile::tempdir().unwrap();
        let cache_subdir = cache_holder.path().join("cache_subdir");
        std::os::unix::fs::symlink(attacker.path(), &cache_subdir).unwrap();

        let dst = cache_subdir.join("target");
        let src_holder = tempfile::tempdir().unwrap();
        std::fs::write(src_holder.path().join("payload"), b"x").unwrap();

        safe_copy_into_new(src_holder.path(), &dst).unwrap_err();

        // Attacker dir must not have received the payload.
        assert!(!attacker.path().join("target").join("payload").exists());
    }

    #[test]
    fn safe_copy_into_existing_copies_at_basename() {
        let src_holder = tempfile::tempdir().unwrap();
        let src = src_holder.path().join("src_basename");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("data.txt"), b"data").unwrap();

        let dst_holder = tempfile::tempdir().unwrap();

        safe_copy_into_existing(&src, dst_holder.path()).unwrap();

        assert_eq!(
            std::fs::read(
                dst_holder
                    .path()
                    .join("src_basename")
                    .join("data.txt")
            )
            .unwrap(),
            b"data"
        );
    }

    #[test]
    fn safe_copy_into_existing_refuses_leaf_symlink() {
        let attacker = tempfile::tempdir().unwrap();
        std::fs::write(attacker.path().join("PWNED"), b"attacker payload").unwrap();

        let chain_cache = tempfile::tempdir().unwrap();
        let state_link = chain_cache.path().join("state");
        std::os::unix::fs::symlink(attacker.path(), &state_link).unwrap();

        let dst_holder = tempfile::tempdir().unwrap();

        safe_copy_into_existing(&state_link, dst_holder.path()).unwrap_err();

        assert!(!dst_holder.path().join("state").join("PWNED").exists());
    }
}
