#[allow(dead_code)]
mod support;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use support::temp_path;

#[cfg(unix)]
const FAKE_ELIO: &str = r#"#!/bin/sh
if [ -n "${ELIO_TEST_ARGS_LOG-}" ]; then
  printf '%s\n' "$*" >> "$ELIO_TEST_ARGS_LOG"
fi

if [ "$1" = "--cwd-file" ]; then
  if [ -n "${ELIO_TEST_TEMP_LOG-}" ]; then
    printf '%s' "$2" > "$ELIO_TEST_TEMP_LOG"
  fi
  if [ "${3-}" = "empty" ]; then
    : > "$2"
    exit 9
  fi
  printf '%s' "$ELIO_TEST_DESTINATION" > "$2"
  exit 7
fi

if [ "$1" = "--version" ] || [ "$1" = "-V" ]; then
  printf 'VERSION-PASSTHROUGH\n'
  exit 0
fi

if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
  printf 'HELP-PASSTHROUGH\n'
  exit 3
fi

if [ "$1" = "shell" ]; then
  printf 'SHELL-PASSTHROUGH\n'
  exit 4
fi

exit 5
"#;

#[cfg(windows)]
const FAKE_ELIO: &str = r#"@echo off
if defined ELIO_TEST_ARGS_LOG >>"%ELIO_TEST_ARGS_LOG%" echo(%*
if "%~1"=="--cwd-file" goto cwd
if "%~1"=="--version" goto version
if "%~1"=="-V" goto version
if "%~1"=="--help" goto help
if "%~1"=="-h" goto help
if "%~1"=="shell" goto shell
exit /b 5

:cwd
if defined ELIO_TEST_TEMP_LOG >"%ELIO_TEST_TEMP_LOG%" <nul set /p "=%~2"
if "%~3"=="empty" goto empty
>"%~2" <nul set /p "=%ELIO_TEST_DESTINATION%"
exit /b 7

:empty
type nul >"%~2"
exit /b 9

:version
echo VERSION-PASSTHROUGH
exit /b 0

:help
echo HELP-PASSTHROUGH
exit /b 3

:shell
echo SHELL-PASSTHROUGH
exit /b 4
"#;

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let path = temp_path(label);
        fs::create_dir_all(&path)?;
        Ok(Self {
            path: resolved_path(&path)?,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
fn resolved_path(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    Ok(path.canonicalize()?)
}

/// Canonical paths carry the `\\?\` verbatim prefix on Windows, which PowerShell
/// would echo back in `$PWD`, so drop it again.
#[cfg(windows)]
fn resolved_path(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let resolved = path.canonicalize()?;
    let text = resolved.to_string_lossy();
    Ok(match text.strip_prefix(r"\\?\") {
        Some(stripped) if !stripped.starts_with("UNC") => PathBuf::from(stripped),
        _ => resolved,
    })
}

#[test]
fn generated_pwsh_function_runs_when_executed() -> Result<(), Box<dyn Error>> {
    if !pwsh_available() {
        return Ok(());
    }

    let root = TempRoot::new("shell-runtime-pwsh")?;
    let start_dir = root.path().join("start");
    let destination = root.path().join("destination with spaces");
    let home_dir = root.path().join("home");
    let config_home = root.path().join("config");
    let args_log = root.path().join("args.log");
    for dir in [&start_dir, &destination, &home_dir, &config_home] {
        fs::create_dir_all(dir)?;
    }

    let init_script = root.path().join("init.ps1");
    fs::write(&init_script, wrapper_calling_fake_elio(root.path())?)?;
    let runtime_script = root.path().join("runtime.ps1");
    fs::write(
        &runtime_script,
        runtime_script_text(&init_script, &start_dir, &destination),
    )?;

    let output = Command::new("pwsh")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
        .arg(&runtime_script)
        .env("PATH", path_with_prefix(&root.path().join("fake-bin"))?)
        .env("ELIO_TEST_DESTINATION", &destination)
        .env("ELIO_TEST_ARGS_LOG", &args_log)
        .env("HOME", &home_dir)
        .env("XDG_CONFIG_HOME", &config_home)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "pwsh runtime script failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.is_empty(),
        "pwsh runtime script printed stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("PWSH-RUNTIME-OK"),
        "pwsh runtime script did not reach the end\nstdout:\n{stdout}"
    );
    Ok(())
}

/// Generates the wrapper the way an installed elio on PATH would, with a fake
/// elio in `fake-bin` standing in for the real executable at run time.
#[cfg(unix)]
fn wrapper_calling_fake_elio(root: &Path) -> Result<String, Box<dyn Error>> {
    use std::{io::Write, os::unix::fs::symlink, process::Stdio};

    let gen_dir = root.join("gen-bin");
    let fake_dir = root.join("fake-bin");
    fs::create_dir_all(&gen_dir)?;
    fs::create_dir_all(&fake_dir)?;
    symlink(env!("CARGO_BIN_EXE_elio"), gen_dir.join("elio"))?;

    // Written by a child process: an executable this process wrote itself can
    // still be open in a concurrently forked test, which makes running it fail.
    let mut writer = Command::new("sh")
        .arg("-c")
        .arg("cat > \"$1\" && chmod +x \"$1\"")
        .arg("sh")
        .arg(fake_dir.join("elio"))
        .stdin(Stdio::piped())
        .spawn()?;
    writer
        .stdin
        .take()
        .expect("writer stdin should be piped")
        .write_all(FAKE_ELIO.as_bytes())?;
    assert!(writer.wait()?.success(), "fake elio should be written");

    let output = Command::new("elio")
        .args(["shell", "init", "pwsh"])
        .env("PATH", path_with_prefix(&gen_dir)?)
        .output()?;
    let script = init_script_from(output)?;
    assert!(
        script.contains("-CommandType Application"),
        "official-install pwsh init script should call elio from PATH:\n{script}"
    );
    assert!(
        !script.contains(env!("CARGO_BIN_EXE_elio")),
        "official-install pwsh init script should not contain the test binary path:\n{script}"
    );
    Ok(script)
}

/// A batch file cannot be named `elio.exe`, so on Windows the wrapper is
/// generated for the real executable's path and pointed at the fake afterwards.
#[cfg(windows)]
fn wrapper_calling_fake_elio(root: &Path) -> Result<String, Box<dyn Error>> {
    let fake_dir = root.join("fake-bin");
    fs::create_dir_all(&fake_dir)?;
    let fake = fake_dir.join("elio.cmd");
    fs::write(&fake, FAKE_ELIO.replace('\n', "\r\n"))?;

    let output = Command::new(env!("CARGO_BIN_EXE_elio"))
        .args(["shell", "init", "pwsh"])
        .output()?;
    let script = init_script_from(output)?;
    let real = format!(
        "$elioExe = {}",
        pwsh_quote(Path::new(env!("CARGO_BIN_EXE_elio")))
    );
    assert!(
        script.contains(&real),
        "path-invoked pwsh init script should call elio by its path:\n{script}"
    );
    Ok(script.replace(&real, &format!("$elioExe = {}", pwsh_quote(&fake))))
}

fn init_script_from(output: std::process::Output) -> Result<String, Box<dyn Error>> {
    assert!(
        output.status.success(),
        "failed to generate pwsh init script\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr while generating pwsh init script:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn runtime_script_text(init_script: &Path, start_dir: &Path, destination: &Path) -> String {
    format!(
        r#"$ErrorActionPreference = 'Stop'
. {init}
$start = {start}
$destination = {destination}

function Assert-Call($name, $code, $location) {{
    if ($LASTEXITCODE -ne $code) {{ throw "${{name}}: expected exit code $code, got $LASTEXITCODE" }}
    $comparison = if ($IsWindows) {{ 'OrdinalIgnoreCase' }} else {{ 'Ordinal' }}
    if (-not [string]::Equals($PWD.Path, $location, $comparison)) {{
        throw "${{name}}: expected location $location, got $($PWD.Path)"
    }}
}}

function Assert-Forwarded($name, $pattern) {{
    $line = @(Get-Content -LiteralPath $env:ELIO_TEST_ARGS_LOG)[-1]
    if ($line -cnotlike $pattern) {{ throw "${{name}}: elio received '$line', expected '$pattern'" }}
}}

Set-Location -LiteralPath $start
elio
Assert-Call 'normal call' 7 $destination
Assert-Forwarded 'normal call' '--cwd-file *'

Set-Location -LiteralPath $start
elio empty
Assert-Call 'empty cwd file' 9 $start

elio --chooser-file /tmp/elio-choice child
Assert-Call 'flag-first chooser' 5 $start
Assert-Forwarded 'flag-first chooser' '--chooser-file /tmp/elio-choice child'
elio child --chooser-file /tmp/elio-choice
Assert-Call 'path-first chooser' 5 $start
Assert-Forwarded 'path-first chooser' 'child --chooser-file /tmp/elio-choice'
elio child --chooser-file=/tmp/elio-choice
Assert-Call 'path-first chooser=' 5 $start
Assert-Forwarded 'path-first chooser=' 'child --chooser-file=/tmp/elio-choice'

$help = elio --help
Assert-Call '--help' 3 $start
if ($help -cne 'HELP-PASSTHROUGH') {{ throw '--help was not passed through' }}
$shell = elio shell status
Assert-Call 'shell subcommand' 4 $start
if ($shell -cne 'SHELL-PASSTHROUGH') {{ throw 'shell subcommand was not passed through' }}
Assert-Forwarded 'shell subcommand' 'shell status'

$version = elio --version
Assert-Call '--version' 0 $start
if ($version -cne 'VERSION-PASSTHROUGH') {{ throw '--version was not passed through' }}
$version = elio -V
Assert-Call '-V' 0 $start
if ($version -cne 'VERSION-PASSTHROUGH') {{ throw '-V was consumed as a PowerShell common parameter' }}

elio -Verbose
Assert-Call '-Verbose' 5 $start
Assert-Forwarded '-Verbose' '-Verbose'
elio -Debug
Assert-Call '-Debug' 5 $start
Assert-Forwarded '-Debug' '-Debug'
elio -ErrorAction Stop
Assert-Call '-ErrorAction' 5 $start
Assert-Forwarded '-ErrorAction' '-ErrorAction Stop'
elio -ea Stop
Assert-Call '-ea' 5 $start
Assert-Forwarded '-ea' '-ea Stop'

elio child -Verbose -Debug -ErrorAction Stop -ea Ignore
Assert-Call 'common parameters after a path' 7 $destination
Assert-Forwarded 'common parameters after a path' '--cwd-file * child -Verbose -Debug -ErrorAction Stop -ea Ignore'

Set-Location -LiteralPath $start
$env:ELIO_TEST_TEMP_LOG = Join-Path $start 'temp-path'
$PSNativeCommandUseErrorActionPreference = $true
$caught = $false
try {{ elio empty }} catch {{ $caught = $true }}
$PSNativeCommandUseErrorActionPreference = $false
if (-not $caught -or $LASTEXITCODE -ne 9) {{ throw 'Terminating error lost the exit code' }}
$tmp = Get-Content -LiteralPath $env:ELIO_TEST_TEMP_LOG -Raw
if (Test-Path -LiteralPath $tmp) {{ throw 'Terminating error leaked the temporary file' }}

$caseStart = Join-Path $start 'start'
$caseDestination = Join-Path $start 'START'
New-Item -ItemType Directory -Force $caseStart, $caseDestination >$null
Set-Content -LiteralPath (Join-Path $caseDestination 'probe') -Value 'case probe'
if (-not (Test-Path -LiteralPath (Join-Path $caseStart 'probe'))) {{
    Set-Location -LiteralPath $caseStart
    $env:ELIO_TEST_DESTINATION = $caseDestination
    elio
    if ($PWD.Path -cne $caseDestination) {{ throw 'Case-only directory change was skipped' }}
}}

'PWSH-RUNTIME-OK'
"#,
        init = pwsh_quote(init_script),
        start = pwsh_quote(start_dir),
        destination = pwsh_quote(destination),
    )
}

fn pwsh_available() -> bool {
    Command::new("pwsh")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn path_with_prefix(prefix: &Path) -> Result<std::ffi::OsString, Box<dyn Error>> {
    let mut paths = vec![prefix.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    Ok(std::env::join_paths(paths)?)
}

fn pwsh_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}
