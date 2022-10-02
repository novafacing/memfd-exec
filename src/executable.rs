//! This essentially reimplements the code at:
//! which is an internal implementation of the code at:
//! https://github.com/rust-lang/rust/blob/master/library/std/src/process.rs
//! https://github.com/rust-lang/rust/blob/master/library/std/src/sys/unix/process/process_unix.rs
//! https://github.com/rust-lang/rust/blob/master/library/std/src/sys/unix/process/process_common.rs
//! for external use to provide a very similar interface to process::Command for in-memory executables

use std::{
    collections::BTreeMap,
    ffi::{CStr, CString, OsStr, OsString},
    io::{Error, ErrorKind, Result},
    mem::MaybeUninit,
    os::unix::prelude::{OsStrExt, OsStringExt},
    path::Path,
    ptr::null_mut,
};

use libc::{pid_t, sigemptyset, signal};
use nix::{
    sys::memfd::{memfd_create, MemFdCreateFlag},
    unistd::{close, fexecve, write},
};

use crate::{
    anon_pipe::anon_pipe,
    child::Child,
    command_env::CommandEnv,
    cvt::{cvt, cvt_nz, cvt_r},
    output::Output,
    process::{ExitStatus, Process},
    stdio::{ChildPipes, Stdio, StdioPipes},
};

/// This is the main struct used to create an in-memory only executable. Wherever possible, it
/// is intended to be a drop-in replacement for the standard library's `process::Command` struct.
///
/// # Examples
///
/// This example is the "motivating case" for this library. It shows how to execute a binary
/// entirely from memory, without writing it to disk. This is useful for executing binaries
/// sneakily, or (the real reason) for bundling executables that are a pain to set up and
/// compile, but whose static versions are very portable. Here's a "sneaky" example:
///
/// ```no_run
/// use memfd_exec::{MemFdExecutable, Stdio};
///
/// // You can include the entirety of a binary (for example, nc)
/// let nc_binary= include_bytes!("/usr/bin/nc-static");
///
///
/// // The first argument is just the name for the program, you can pick anything but
/// // if the program expects a specific argv[0] value, use that.
/// // The second argument is the binary code to execute.
/// let mut cmd = MemFdExecutable::new("nc", nc_binary)
///     // We can pass arbitrary args just like with Command. Here, we'll execute nc
///     // to listen on a port and run a shell for connections, entirely from memory.
///     .arg("-l")
///     .arg("1234")
///     .arg("-e")
///     .arg("/bin/sh")
///     // And we can get piped stdin/stdout just like with Command
///     .stdout(Stdio::piped())
///     // Spawn starts the child process and gives us a handle back
///     .spawn()
///     .expect("failed to execute process");
///
/// // Then, we can wait for the program to exit.
/// cmd.wait();
/// ```
pub struct MemFdExecutable {
    /// The contents of the ELF executable to run. This content can be included in the file
    /// using the `include_bytes!()` macro, or you can do fancy things like read it in from
    /// a socket.
    code: Vec<u8>,
    /// The name of the program, this value is the argv[0] argument to the binary when
    /// executed. If the program expects something specific here, that value should be
    /// used, otherwise any name will do
    program: CString,
    /// The arguments to the program, excluding the program name
    args: Vec<CString>,
    /// The whole argv array, including the program name
    argv: Argv,
    /// The environment variables to set for the program
    env: CommandEnv,
    /// The current working directory to set for the program
    cwd: Option<CString>,
    /// The program's stdin handle
    pub stdin: Option<Stdio>,
    /// The program's stdout handle
    pub stdout: Option<Stdio>,
    /// The program's stderr handle
    pub stderr: Option<Stdio>,
    /// Holdover from Command, whether there was a NUL in the arguments or not
    saw_nul: bool,
}

struct Argv(Vec<CString>);

unsafe impl Send for Argv {}
unsafe impl Sync for Argv {}

fn os2c(s: &OsStr, saw_nul: &mut bool) -> CString {
    CString::new(s.as_bytes()).unwrap_or_else(|_e| {
        *saw_nul = true;
        CString::new("<string-with-nul>").unwrap()
    })
}

fn construct_envp(env: BTreeMap<OsString, OsString>, saw_nul: &mut bool) -> Vec<CString> {
    let mut result = Vec::with_capacity(env.len());
    for (mut k, v) in env {
        // Reserve additional space for '=' and null terminator
        k.reserve_exact(v.len() + 2);
        k.push("=");
        k.push(&v);

        // Add the new entry into the array
        if let Ok(item) = CString::new(k.into_vec()) {
            result.push(item);
        } else {
            *saw_nul = true;
        }
    }

    result
}

impl MemFdExecutable {
    /// Create a new MemFdExecutable with the given name and code. The name is the name of the
    /// program, and is used as the argv[0] argument to the program. The code is the binary
    /// code to execute (usually, the entire contents of an ELF file).
    ///
    /// # Examples
    ///
    /// You can run code that is included directly in your executable with `include_bytes!()`:
    ///
    /// ```no_run
    ///
    /// use memfd_exec::MemFdExecutable;
    ///
    /// let code = include_bytes!("/usr/bin/nc-static");
    ///
    /// let mut cmd = MemFdExecutable::new("nc", code);
    ///     .arg("-l")
    ///     .arg("1234")
    ///     .arg("-e")
    ///     .arg("/bin/sh")
    ///     .status()
    ///     .expect("failed to execute process");
    /// ```
    ///
    pub fn new<S: AsRef<OsStr>>(name: S, code: Vec<u8>) -> Self {
        Self::_new(name.as_ref(), code)
    }

    /// Add an argument to the program. This is equivalent to `Command::arg()`.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self._arg(arg.as_ref());
        self
    }

    /// Add multiple arguments to the program. This is equivalent to `Command::args()`.
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg.as_ref());
        }
        self
    }

    /// Add an environment variable to the program. This is equivalent to `Command::env()`.
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self._env_mut().set(key.as_ref(), val.as_ref());
        self
    }

    /// Add multiple environment variables to the program. This is equivalent to `Command::envs()`.
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (ref key, ref val) in vars {
            self._env_mut().set(key.as_ref(), val.as_ref());
        }
        self
    }

    /// Remove an environment variable from the program. This is equivalent to `Command::env_remove()`.
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
        self._env_mut().remove(key.as_ref());
        self
    }

    /// Clear all environment variables from the program. This is equivalent to `Command::env_clear()`.
    pub fn env_clear(&mut self) -> &mut Self {
        self._env_mut().clear();
        self
    }

    /// Set the current working directory for the program. This is equivalent to `Command::current_dir()`.
    pub fn cwd<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self._cwd(dir.as_ref().as_ref());
        self
    }

    /// Set the stdin handle for the program. This is equivalent to `Command::stdin()`. The
    /// default is to inherit the current process's stdin. Note that this `Stdio` is not the
    /// same exactly as `process::Stdio`, but it is feature-equivalent.
    ///
    /// # Examples
    ///
    /// This example creates a `cat` process that will read in the contents passed to its
    /// stdin handle and write them to a null stdout (i.e. it will be discarded). The same
    /// methodology can be used to read from stderr/stdout.
    ///
    /// ```no_run
    /// use std::thread::spawn;
    ///
    /// use memfd_exec::{MemFdExecutable, Stdio};
    ///
    /// let mut cat_cmd = MemFdExecutable::new("cat", include_bytes!("/bin/cat").to_vec())
    ///    .stdin(Stdio::piped())
    ///    .stdout(Stdio::null())
    ///    .spawn()
    ///    .expect("failed to spawn cat");
    ///
    /// let mut cat_stdin = cat_cmd.stdin.take().expect("failed to open stdin");
    /// spawn(move || {
    ///    cat_stdin.write_all(b"hello world").expect("failed to write to stdin");
    /// });
    /// ```
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self._stdin(cfg.into());
        self
    }

    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self._stdout(cfg.into());
        self
    }

    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self._stderr(cfg.into());
        self
    }

    /// Spawn the program as a child process. This is equivalent to `Command::spawn()`.
    pub fn spawn(&mut self) -> Result<Child> {
        self._spawn(Stdio::Inherit, true)
    }

    /// Spawn the program as a child process and wait for it to complete, obtaining the
    /// output and exit status. This is equivalent to `Command::output()`.
    pub fn output(&mut self) -> Result<Output> {
        self.spawn()?.wait_with_output()
    }

    /// Spawn the program as a child process and wait for it to complete, obtaining the
    /// exit status. This is equivalent to `Command::status()`.
    pub fn status(&mut self) -> Result<ExitStatus> {
        self.spawn()?.wait()
    }

    /// Create a new `Executable` from the given program name.
    pub fn _new(name: &OsStr, code: Vec<u8>) -> Self {
        let mut saw_nul = false;
        let name = os2c(name, &mut saw_nul);
        Self {
            code,
            program: name.clone(),
            args: vec![name.clone()],
            argv: Argv(vec![name]),
            env: Default::default(),
            cwd: None,
            stdin: None,
            stdout: None,
            stderr: None,
            saw_nul,
        }
    }

    fn _set_program(&mut self, program: &OsStr) {
        let arg = os2c(program, &mut self.saw_nul);
        self.argv.0[0] = arg.clone();
        self.args[0] = arg;
    }

    fn _arg(&mut self, arg: &OsStr) {
        let arg = os2c(arg, &mut self.saw_nul);
        self.argv.0.push(arg.clone());
        self.args.push(arg);
    }

    fn _cwd(&mut self, dir: &OsStr) {
        self.cwd = Some(os2c(dir, &mut self.saw_nul));
    }

    fn _stdin(&mut self, stdin: Stdio) {
        self.stdin = Some(stdin);
    }

    fn _stdout(&mut self, stdout: Stdio) {
        self.stdout = Some(stdout);
    }

    fn _stderr(&mut self, stderr: Stdio) {
        self.stderr = Some(stderr);
    }

    fn _env_mut(&mut self) -> &mut CommandEnv {
        &mut self.env
    }

    fn _setup_io(&self, default: Stdio, needs_stdin: bool) -> Result<(StdioPipes, ChildPipes)> {
        let null = Stdio::Null;
        let default_stdin = if needs_stdin { &default } else { &null };
        let stdin = self.stdin.as_ref().unwrap_or(default_stdin);
        let stdout = self.stdout.as_ref().unwrap_or(&default);
        let stderr = self.stderr.as_ref().unwrap_or(&default);
        let (their_stdin, our_stdin) = stdin.to_child_stdio(true)?;
        let (their_stdout, our_stdout) = stdout.to_child_stdio(false)?;
        let (their_stderr, our_stderr) = stderr.to_child_stdio(false)?;
        let ours = StdioPipes {
            stdin: our_stdin,
            stdout: our_stdout,
            stderr: our_stderr,
        };
        let theirs = ChildPipes {
            stdin: their_stdin,
            stdout: their_stdout,
            stderr: their_stderr,
        };
        Ok((ours, theirs))
    }

    pub fn _saw_nul(&self) -> bool {
        self.saw_nul
    }

    pub fn _get_cwd(&self) -> &Option<CString> {
        &self.cwd
    }

    pub fn _spawn(&mut self, default: Stdio, needs_stdin: bool) -> Result<Child> {
        const CLOEXEC_MSG_FOOTER: [u8; 4] = *b"NOEX";

        let envp = self._capture_env();

        if self._saw_nul() {
            // TODO: Need err?
        }

        let (ours, theirs) = self._setup_io(default, needs_stdin)?;

        let (input, output) = anon_pipe()?;

        // Whatever happens after the fork is almost for sure going to touch or
        // look at the environment in one way or another (PATH in `execvp` or
        // accessing the `environ` pointer ourselves). Make sure no other thread
        // is accessing the environment when we do the fork itself.
        //
        // Note that as soon as we're done with the fork there's no need to hold
        // a lock any more because the parent won't do anything and the child is
        // in its own process. Thus the parent drops the lock guard while the child
        // forgets it to avoid unlocking it on a new thread, which would be invalid.
        // TODO: Yeah....I had to remove the env lock. Whoops! Don't multithread env with this
        // you insane person
        let pid = unsafe { self._do_fork()? };

        if pid == 0 {
            drop(input);
            let Err(err) = (unsafe { self._do_exec(theirs, envp) }) else { unreachable!("..."); };
            panic!("failed to exec: {}", err);
        }

        drop(output);

        // Safety: We obtained the pidfd from calling `clone3` with
        // `CLONE_PIDFD` so it's valid an otherwise unowned.
        let mut p = unsafe { Process::new(pid) };
        let mut bytes = [0; 8];

        // loop to handle EINTR
        loop {
            match input.read(&mut bytes) {
                Ok(0) => return Ok(Child::new(p, ours)),
                Ok(8) => {
                    let (errno, footer) = bytes.split_at(4);
                    assert_eq!(
                        CLOEXEC_MSG_FOOTER, footer,
                        "Validation on the CLOEXEC pipe failed: {:?}",
                        bytes
                    );
                    let errno = i32::from_be_bytes(errno.try_into().unwrap());
                    assert!(p.wait().is_ok(), "wait() should either return Ok or panic");
                    return Err(Error::from_raw_os_error(errno));
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => {
                    assert!(p.wait().is_ok(), "wait() should either return Ok or panic");
                    panic!("the CLOEXEC pipe failed: {e:?}")
                }
                Ok(..) => {
                    // pipe I/O up to PIPE_BUF bytes should be atomic
                    assert!(p.wait().is_ok(), "wait() should either return Ok or panic");
                    panic!("short read on the CLOEXEC pipe")
                }
            }
        }
    }

    unsafe fn _do_fork(&mut self) -> Result<pid_t> {
        cvt(libc::fork())
    }

    pub fn _capture_env(&mut self) -> Option<Vec<CString>> {
        let maybe_env = self.env.capture_if_changed();
        maybe_env.map(|env| construct_envp(env, &mut self.saw_nul))
    }

    pub fn _exec(&mut self, default: Stdio) -> Error {
        let envp = self._capture_env();

        if self._saw_nul() {
            return Error::new(ErrorKind::InvalidInput, "nul byte found in provided data");
        }

        match self._setup_io(default, true) {
            Ok((_, theirs)) => unsafe {
                let Err(e) = self._do_exec(theirs, envp) else { unreachable!("..."); };
                e
            },
            Err(e) => e,
        }
    }

    pub fn _get_program_cstr(&self) -> &CStr {
        &*self.program
    }

    pub fn _get_argv(&self) -> &Vec<CString> {
        &self.argv.0
    }

    pub fn _env_saw_path(&self) -> bool {
        self.env.have_changed_path()
    }

    pub fn _program_is_path(&self) -> bool {
        self.program.to_bytes().contains(&b'/')
    }

    unsafe fn _do_exec(
        &mut self,
        stdio: ChildPipes,
        maybe_envp: Option<Vec<CString>>,
    ) -> Result<!> {
        if let Some(fd) = stdio.stdin.fd() {
            cvt_r(|| libc::dup2(fd, libc::STDIN_FILENO))?;
        }
        if let Some(fd) = stdio.stdout.fd() {
            cvt_r(|| libc::dup2(fd, libc::STDOUT_FILENO))?;
        }
        if let Some(fd) = stdio.stderr.fd() {
            cvt_r(|| libc::dup2(fd, libc::STDERR_FILENO))?;
        }

        if let Some(ref cwd) = *self._get_cwd() {
            cvt(libc::chdir(cwd.as_ptr()))?;
        }

        {
            // Reset signal handling so the child process starts in a
            // standardized state. libstd ignores SIGPIPE, and signal-handling
            // libraries often set a mask. Child processes inherit ignored
            // signals and the signal mask from their parent, but most
            // UNIX programs do not reset these things on their own, so we
            // need to clean things up now to avoid confusing the program
            // we're about to run.
            let mut set = MaybeUninit::<libc::sigset_t>::uninit();
            cvt(sigemptyset(set.as_mut_ptr()))?;
            cvt_nz(libc::pthread_sigmask(
                libc::SIG_SETMASK,
                set.as_ptr(),
                null_mut(),
            ))?;

            {
                let ret = signal(libc::SIGPIPE, libc::SIG_DFL);
                if ret == libc::SIG_ERR {
                    return Err(Error::last_os_error());
                }
            }
        }

        // TODO: Env resetting isn't implemented because we're using fexecve not execvp

        // Map the executable last, because it's a huge hit to memory if something else failed
        let mfd = memfd_create(
            CString::new("rust_exec").unwrap().as_c_str(),
            MemFdCreateFlag::MFD_CLOEXEC,
        )
        .unwrap();

        if let Ok(n) = write(mfd, &self.code[..]) {
            if n != self.code.len() {
                return Err(Error::new(
                    ErrorKind::BrokenPipe,
                    "Failed to write to memfd",
                ));
            }
        } else {
            return Err(Error::last_os_error());
        }

        let argv = self
            ._get_argv()
            .iter()
            .map(|s| s.as_c_str())
            .collect::<Vec<_>>();

        let maybe_envp = maybe_envp.unwrap_or_default();

        let envp = maybe_envp.iter().map(|s| s.as_c_str()).collect::<Vec<_>>();

        if let Err(err) = fexecve(mfd, &argv, &envp) {
            // If we failed to exec, we need to close the memfd
            // so that the child process doesn't leak it
            let _ = close(mfd);
            return Err(Error::new(ErrorKind::BrokenPipe, err));
        }
        Err(Error::last_os_error())
    }
}
