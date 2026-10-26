use std::io::{self, ErrorKind, Read, Write};
use std::thread;
use std::time::Duration;

use interprocess::local_socket::traits::{ListenerExt as _, Stream as _};
use interprocess::local_socket::{ListenerOptions, Name, Stream};
use tokio::sync::mpsc::UnboundedSender;

const SOCKET: &str = match cfg!(debug_assertions) {
    true => "sonora-dev.sock",
    false => "sonora.sock",
};

pub enum Instance {
    First,
    Running,
    Failed,
}

/// `args` is every non-flag command-line argument of this launch: a lone `spotify:`/web link, or
/// one or more file paths from an "Open With" selection.
pub fn claim(args: &[String], sender: UnboundedSender<Vec<String>>) -> Instance {
    let name = match name() {
        Ok(name) => name,
        Err(error) => {
            log::warn!("single: cannot name the instance socket: {error:#}");
            return Instance::Failed;
        }
    };

    let listener = match options(name.clone()).create_sync() {
        Ok(listener) => listener,

        Err(error) => {
            // Windows reports an occupied pipe with an error other than
            // AddrInUse, so any failure may mean another Sonora owns the socket.
            if hand_over(name.clone(), args) {
                return Instance::Running;
            }

            // Only a filesystem socket can leave a stale AddrInUse behind.
            if error.kind() != ErrorKind::AddrInUse {
                log::error!("single: cannot own the instance socket: {error:#}");
                return Instance::Failed;
            }

            log::warn!("single: socket is occupied but unreachable; attempting recovery");

            match options(name.clone())
                .try_overwrite(true)
                .max_spin_time(Duration::ZERO)
                .create_sync()
            {
                Ok(listener) => listener,
                Err(error) => {
                    log::error!("single: cannot recover instance socket: {error:#}");
                    return Instance::Failed;
                }
            }
        }
    };

    thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut connection) = connection else {
                continue;
            };
            let mut payload = String::new();
            if connection.read_to_string(&mut payload).is_err() {
                continue;
            }
            let items: Vec<String> = payload
                .split('\n')
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect();
            if sender.send(items).is_err() {
                break;
            }
        }
    });

    Instance::First
}

fn hand_over(name: Name<'_>, args: &[String]) -> bool {
    let mut stream = match Stream::connect(name) {
        Ok(stream) => stream,
        Err(error) => {
            log::warn!("single: cannot reach the running instance: {error:#}");
            return false;
        }
    };
    stream.write_all(args.join("\n").as_bytes()).is_ok()
}

/// The instance socket as a file in a folder only this user can enter, so another account can
/// neither take the name first nor connect to it.
#[cfg(unix)]
fn name() -> io::Result<Name<'static>> {
    use interprocess::local_socket::{GenericFilePath, ToFsName as _};

    private_dir()?.join(SOCKET).to_fs_name::<GenericFilePath>()
}

/// The instance socket as a named pipe, which `reachable` closes to other accounts.
#[cfg(windows)]
fn name() -> io::Result<Name<'static>> {
    use interprocess::local_socket::{GenericNamespaced, ToNsName as _};

    SOCKET.to_ns_name::<GenericNamespaced>()
}

/// The user's runtime folder, which the session manager creates readable by this user alone. A
/// Flatpak sandbox gets a fresh one per launch, so there the app's own subfolder, shared by every
/// launch, is used instead. Without a runtime folder the socket goes in the user's cache folder.
#[cfg(all(unix, not(target_os = "macos")))]
fn private_dir() -> io::Result<std::path::PathBuf> {
    use std::os::unix::fs::DirBuilderExt as _;
    use std::path::PathBuf;

    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from);
    let dir = match (runtime, std::env::var_os("FLATPAK_ID")) {
        (Some(runtime), Some(app)) => runtime.join("app").join(app),
        (Some(runtime), None) => return Ok(runtime),
        (None, _) => dirs::cache_dir()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "no runtime or cache folder"))?
            .join("sonora"),
    };
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    Ok(dir)
}

/// The per-user temporary folder macOS keeps under `/var/folders`, which only this user can
/// enter. It is asked of the system rather than read from `TMPDIR`, so a launch from a shell
/// that changed the variable still finds the same socket.
#[cfg(target_os = "macos")]
fn private_dir() -> io::Result<std::path::PathBuf> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt as _;

    let mut buffer = vec![0u8; libc::PATH_MAX as usize];
    // SAFETY: confstr writes at most `buffer.len()` bytes into the buffer it is given.
    let length = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    if length == 0 || length > buffer.len() {
        return Err(io::Error::last_os_error());
    }
    // the length counts the terminating NUL
    buffer.truncate(length - 1);
    Ok(OsString::from_vec(buffer).into())
}

/// The listener for the instance socket, open to every Sonora this user launches.
fn options(name: Name<'_>) -> ListenerOptions<'_> {
    let options = ListenerOptions::new().name(name);
    #[cfg(windows)]
    let options = match reachable() {
        Some(descriptor) => {
            use interprocess::os::windows::local_socket::ListenerOptionsExt as _;
            options.security_descriptor(descriptor)
        }
        None => options,
    };
    options
}

/// Access to the pipe for this user's account alone, elevated or not. Windows owns an elevated
/// process's objects as Administrators, so neither the default descriptor nor one granting the
/// owner lets the same user's taskbar launch reach a Sonora the updater relaunched elevated, and
/// that launch would exit without a window. `None` keeps the default, and is logged.
#[cfg(windows)]
fn reachable() -> Option<interprocess::os::windows::security_descriptor::SecurityDescriptor> {
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;

    let described = user_sid()
        .and_then(|sid| {
            widestring::U16CString::from_str(format!("D:(A;;GA;;;{sid})"))
                .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))
        })
        .and_then(|descriptor| SecurityDescriptor::deserialize(&descriptor));
    match described {
        Ok(descriptor) => Some(descriptor),
        Err(error) => {
            log::warn!("single: cannot describe the socket's access: {error:#}");
            None
        }
    }
}

/// The SID of the account this process runs as, in its `S-1-5-…` text form.
#[cfg(windows)]
fn user_sid() -> io::Result<String> {
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: the pseudo handle of the current process needs no closing, and `token` is a live
    // out pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut length = 0u32;
    // SAFETY: a null buffer of length zero only asks for the size the answer needs.
    unsafe { GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut length) };
    // u64 cells keep the buffer aligned for the pointer at the start of TOKEN_USER
    let mut buffer = vec![0u64; (length as usize).div_ceil(size_of::<u64>())];
    // SAFETY: the buffer holds at least `length` bytes, and the token is open until closed below.
    let read = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    };
    let error = io::Error::last_os_error();
    // SAFETY: the token was opened above and is not used again.
    unsafe { CloseHandle(token) };
    if read == 0 {
        return Err(error);
    }

    // SAFETY: GetTokenInformation filled the buffer with a TOKEN_USER whose SID points into it.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text = ptr::null_mut();
    // SAFETY: `sid` is valid while `buffer` lives, and `text` is a live out pointer.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ConvertSidToStringSidW returned a NUL-terminated string that LocalFree releases.
    let sid = unsafe { widestring::U16CStr::from_ptr_str(text) }.to_string_lossy();
    unsafe { LocalFree(text.cast()) };
    Ok(sid)
}
