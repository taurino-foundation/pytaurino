use std::{
    env, fs,
    io::{self, Error, ErrorKind},
    path::{Path, PathBuf},
    sync::OnceLock,
};

static STARTING_BINARY: OnceLock<StartingBinary> = OnceLock::new();

/// Represents the cached executable path of the running application.
///
/// The path is resolved and canonicalized once, on first initialization.
/// Both successful results and failures are cached for the lifetime of
/// the process.
#[derive(Debug)]
pub struct StartingBinary {
    path: io::Result<PathBuf>,
}

impl StartingBinary {
    /// Initializes the executable path cache.
    ///
    /// Initialization is thread-safe and occurs at most once.
    ///
    /// For predictable startup behavior, call this function as early
    /// as possible in `main`, before starting worker threads.
    pub fn initialize() -> &'static Self { STARTING_BINARY.get_or_init(Self::new) }

    /// Returns a borrowed reference to the cached executable path.
    ///
    /// The returned path remains valid for the lifetime of the process.
    pub fn get() -> io::Result<&'static Path> {
        Self::initialize().path.as_ref().map(PathBuf::as_path).map_err(clone_io_error)
    }

    /// Returns an owned copy of the cached executable path.
    pub fn cloned() -> io::Result<PathBuf> { Self::get().map(Path::to_path_buf) }

    /// Returns the parent directory of the cached executable path.
    pub fn directory() -> io::Result<PathBuf> {
        Self::get()?
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "Executable parent directory not found"))
    }

    fn new() -> Self { Self { path: resolve_executable() } }
}

/// Resolves the running executable's path.
///
/// The path is validated according to platform-specific rules before
/// being canonicalized.
fn resolve_executable() -> io::Result<PathBuf> {
    let path = env::current_exe()?;

    validate_executable_path(&path)?;

    fs::canonicalize(path)
}

/// Validates the executable path against platform-specific restrictions.
///
/// On macOS, symbolic links in any reported path component are rejected.
///
/// On Linux and Windows, no additional symlink restriction is applied.
/// Symbolic links are resolved during canonicalization instead.
fn validate_executable_path(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        for component in path.ancestors() {
            if fs::symlink_metadata(component)?.file_type().is_symlink() {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Executable path contains a symbolic link: {}", component.display()),
                ));
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
    }

    Ok(())
}

/// Recreates an I/O error for returning a cached failure.
///
/// `std::io::Error` does not implement `Clone`.
/// The error kind and description are preserved, but additional
/// OS-specific or underlying error information may be lost.
fn clone_io_error(error: &io::Error) -> io::Error { Error::new(error.kind(), error.to_string()) }

/// Retrieves the currently running executable's path, taking into account
/// platform-specific behavior and security considerations.
///
/// The path is resolved, canonicalized, and cached on the first call to
/// [`StartingBinary::initialize`] or any function that initializes it.
///
/// Unlike initialization through a static constructor, this cache is not
/// automatically populated before `main` runs. Applications should
/// initialize it as early as possible during startup.
///
/// The cached result is reused for subsequent calls, avoiding repeated
/// executable path resolution.
///
/// # Platform-specific behavior
///
/// ## Windows
///
/// The executable path is obtained using [`std::env::current_exe`] and
/// canonicalized.
///
/// Symbolic links are not explicitly rejected. Canonicalization resolves
/// symbolic links where supported by the filesystem.
///
/// ## Linux
///
/// The executable path is obtained using [`std::env::current_exe`] and
/// canonicalized.
///
/// Symbolic links are allowed and resolved during canonicalization.
///
/// Linux may report a path that no longer exists if the executable was
/// deleted or replaced while the process was running. In such cases,
/// canonicalization may fail.
///
/// ## macOS
///
/// The executable path is rejected if any component of the path reported
/// by [`std::env::current_exe`] is a symbolic link.
///
/// This restriction is intended to reduce risks associated with
/// executable path manipulation.
///
/// There is no feature flag to disable this restriction.
///
/// # Security
///
/// The returned path must not be treated as proof of executable identity.
///
/// The implementation applies the following precautions:
///
/// - Resolves the executable path only once.
/// - Canonicalizes the path to resolve symbolic links.
/// - Rejects detected symbolic links on macOS.
/// - Caches both successful and failed resolution attempts.
///
/// These precautions reduce repeated filesystem resolution and some
/// path-manipulation risks, but do not eliminate all attack scenarios.
///
/// ## Symbolic links
///
/// [`Path::canonicalize`] resolves symbolic links, including nested links.
///
/// On macOS, detected symbolic links in the reported path are rejected
/// before canonicalization.
///
/// On Windows and Linux, symbolic links are resolved rather than rejected.
///
/// Note that the path reported by the operating system may already have
/// resolved some symbolic links. Consequently, the macOS check cannot
/// prove that the original invocation contained no symbolic links.
///
/// ## Hard links
///
/// Hard links reference the same underlying file through different
/// filesystem directory entries.
///
/// Canonicalization does not distinguish between hard links and cannot
/// establish which directory entry was originally used to launch
/// the executable.
///
/// Hard-link creation permissions and restrictions vary by operating
/// system, filesystem, and system configuration.
///
/// ## Time-of-check to time-of-use
///
/// A cached executable path does not guarantee that the filesystem entry
/// will continue to reference the same executable.
///
/// Another process may rename, delete, or replace a file after its path
/// has been resolved.
///
/// Applications that use this path to relaunch an executable must
/// consider this risk, especially when running with elevated privileges.
///
/// ## Errors
///
/// Returns an error if:
///
/// - The operating system cannot determine the executable path.
/// - A required filesystem operation fails.
/// - A symbolic link is detected in the reported path on macOS.
/// - The path cannot be canonicalized.
///
/// The first resolution result, including an error, is cached.
///
/// # Examples
///
/// ```no_run
/// use taurino_config::starting_binary::current_exe;
///
/// fn main() -> std::io::Result<()> {
///     let path = current_exe()?;
///
///     println!("Executable: {}", path.display());
///
///     Ok(())
/// }
/// ```
///
/// # See also
///
/// - [`std::env::current_exe`]
/// - [`std::path::Path::canonicalize`]
pub fn current_exe() -> io::Result<PathBuf> { StartingBinary::cloned() }
