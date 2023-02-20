use anyhow::{Context, Result};
#[cfg(target_os = "linux")]
use libc;
use std::fs;
use std::io;
use std::io::Write;
use std::os::unix::fs::chroot;
use std::process::exit;
use tempfile::tempdir;

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let command = &args[3];
    let command_args = &args[4..];
    let root = tempdir()?;
    let relative_command = match command.strip_prefix("/") {
        Some(p) => p,
        _ => command,
    };
    if let Some(d) = root.path().join(relative_command).parent() {
        fs::create_dir_all(d)?;
    };
    fs::copy(command, root.path().join(relative_command))?;
    chroot(&root)?;
    std::env::set_current_dir("/")?;
    fs::create_dir("/dev")?;
    fs::File::create("/dev/null")?;

    #[cfg(target_os = "linux")]
    unsafe {
        libc::unshare(libc::CLONE_NEWPID);
    }

    let output = std::process::Command::new(command)
        .args(command_args)
        .output()
        .with_context(|| {
            format!(
                "Tried to run '{}' with arguments {:?}",
                command, command_args
            )
        })?;

    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;

    match output.status.code() {
        Some(n) => exit(n),
        _ => exit(-1),
    }
}
