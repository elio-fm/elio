use std::path::Path;

use super::Shell;

pub(crate) fn binary_command(shell: Shell, invocation: Option<&str>, executable: &Path) -> String {
    match shell {
        Shell::Bash | Shell::Zsh | Shell::Fish => posix_binary_command(invocation, executable),
        Shell::Nu => nu_binary_command(invocation, executable),
        Shell::Pwsh => pwsh_binary_command(invocation, executable),
    }
}

fn posix_binary_command(invocation: Option<&str>, executable: &Path) -> String {
    let Some(invocation) = invocation else {
        return shell_quote(executable);
    };

    if invocation.contains('/') || invocation.contains('\\') || invocation.starts_with('.') {
        shell_quote(executable)
    } else {
        "command elio".to_string()
    }
}

fn nu_binary_command(invocation: Option<&str>, executable: &Path) -> String {
    let Some(invocation) = invocation else {
        return nu_string_literal(executable);
    };

    if invocation.contains('/') || invocation.contains('\\') || invocation.starts_with('.') {
        nu_string_literal(executable)
    } else {
        r#""elio""#.to_string()
    }
}

/// PowerShell resolves a bare command name through functions before applications,
/// so the wrapper cannot call `elio` by name without recursing into itself. A bare
/// invocation is looked up as an application instead. pwsh passes the resolved path
/// as argv[0], so scripts generated from pwsh pin the executable path.
fn pwsh_binary_command(invocation: Option<&str>, executable: &Path) -> String {
    let Some(invocation) = invocation else {
        return pwsh_string_literal(executable);
    };

    if invocation.contains('/') || invocation.contains('\\') || invocation.starts_with('.') {
        pwsh_string_literal(executable)
    } else {
        let name = executable
            .file_name()
            .map(|name| pwsh_string_literal(Path::new(name)))
            .unwrap_or_else(|| "'elio'".to_string());
        format!(
            "(Get-Command -Name {name} -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source)"
        )
    }
}

pub(crate) fn init_script(shell: Shell, binary: &str) -> String {
    match shell {
        Shell::Bash | Shell::Zsh => posix_init_script(binary),
        Shell::Fish => fish_init_script(binary),
        Shell::Nu => nu_init_script(binary),
        Shell::Pwsh => pwsh_init_script(binary),
    }
}
fn posix_init_script(executable: &str) -> String {
    format!(
        r#"elio() {{
    case "${{1-}}" in
        shell|portal|-*)
            {executable} "$@"
            return $?
            ;;
    esac

    local arg tmp cwd status_code
    for arg in "$@"; do
        case "$arg" in
            --chooser-file|--chooser-file=*)
                {executable} "$@"
                return $?
                ;;
        esac
    done

    tmp="$(mktemp -t "elio-cwd.XXXXXX")" || return
    {executable} --cwd-file "$tmp" "$@"
    status_code=$?

    if [ -s "$tmp" ]; then
        cwd="$(cat -- "$tmp")"
        rm -f -- "$tmp"
        if [ -n "$cwd" ] && [ "$cwd" != "$PWD" ] && [ -d "$cwd" ]; then
            cd -- "$cwd" || return $?
        fi
    else
        rm -f -- "$tmp"
    fi

    return "$status_code"
}}
"#
    )
}

fn fish_init_script(executable: &str) -> String {
    format!(
        r#"function elio
    switch "$argv[1]"
        case shell portal '-*'
            {executable} $argv
            return $status
    end

    for arg in $argv
        switch "$arg"
            case --chooser-file '--chooser-file=*'
                {executable} $argv
                return $status
        end
    end

    set -l tmp (mktemp -t "elio-cwd.XXXXXX")
    or return

    {executable} --cwd-file "$tmp" $argv
    set -l status_code $status

    if test -s "$tmp"
        set -l cwd (string collect < "$tmp")
        rm -f "$tmp"
        if test -n "$cwd"; and test "$cwd" != "$PWD"; and test -d "$cwd"
            cd "$cwd"; or return $status
        end
    else
        rm -f "$tmp"
    end

    return $status_code
end
"#
    )
}

fn nu_init_script(executable: &str) -> String {
    format!(
        r#"def --env --wrapped elio [...args] {{
  let has_chooser_file = ($args | any {{|arg|
    let value = ($arg | into string)
    ($value == '--chooser-file') or ($value | str starts-with '--chooser-file=')
  }})

  if $has_chooser_file {{
    let status_code = (
      try {{
        run-external {executable} ...$args
        $env.LAST_EXIT_CODE
      }} catch {{|e| ($e.exit_code? | default 127) }}
    )

    $env.LAST_EXIT_CODE = $status_code
    return
  }}

  if (($args | length) > 0) and (
    (($args.0 | into string) == 'shell') or
    (($args.0 | into string) == 'portal') or
    (($args.0 | into string) | str starts-with '-')
  ) {{
    let result = (
      try {{
        run-external {executable} ...$args | complete
      }} catch {{|e| {{ stdout: "", stderr: ($e.msg? | default ""), exit_code: ($e.exit_code? | default 127) }} }}
    )

    if ($result.stderr | is-not-empty) {{
      print -e --no-newline $result.stderr
    }}

    $env.LAST_EXIT_CODE = $result.exit_code

    if (is-terminal --stdout) {{
      if ($result.stdout | is-not-empty) {{
        print --no-newline $result.stdout
      }}
      return
    }}

    return $result.stdout
  }}

  let tmp = (mktemp -t "elio-cwd.XXXXXX")
  let command_args = (["--cwd-file", $tmp] ++ $args)

  let status_code = (
    try {{
      run-external {executable} ...$command_args
      $env.LAST_EXIT_CODE
    }} catch {{|e| ($e.exit_code? | default 127) }}
  )

  let cwd = if ($tmp | path exists) {{ open --raw $tmp }} else {{ "" }}
  rm -f $tmp

  if ($cwd | is-not-empty) and ($cwd != $env.PWD) and (($cwd | path expand | path type) == 'dir') {{
    cd $cwd
  }}

  $env.LAST_EXIT_CODE = $status_code
}}
"#
    )
}

fn pwsh_init_script(executable: &str) -> String {
    format!(
        r#"function elio {{
    $elioExe = {executable}
    if (-not $elioExe) {{
        $global:LASTEXITCODE = 127
        Write-Error 'elio: could not find the elio executable'
        return
    }}

    if ($args.Count -gt 0) {{
        $first = [string]$args[0]
        if ($first -ceq 'shell' -or $first -ceq 'portal' -or $first.StartsWith('-')) {{
            & $elioExe @args
            return
        }}
    }}

    foreach ($arg in $args) {{
        $value = [string]$arg
        if ($value -ceq '--chooser-file' -or $value.StartsWith('--chooser-file=')) {{
            & $elioExe @args
            return
        }}
    }}

    $tmp = [System.IO.Path]::GetTempFileName()
    try {{
        & $elioExe --cwd-file $tmp @args
        $statusCode = $LASTEXITCODE
        $cwd = (Get-Content -LiteralPath $tmp -Raw -ErrorAction SilentlyContinue)
        if ($cwd) {{
            $cwd = $cwd.TrimEnd([char]13, [char]10)
        }}
        if ($cwd -and $cwd -cne $PWD.Path -and (Test-Path -LiteralPath $cwd -PathType Container)) {{
            Set-Location -LiteralPath $cwd
        }}
    }} finally {{
        Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
    }}

    $global:LASTEXITCODE = $statusCode
}}
"#
    )
}

pub(super) fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn nu_string_literal(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// PowerShell single-quoted strings are literal apart from `''`, which escapes a
/// quote, so Windows path separators need no special handling.
pub(super) fn pwsh_string_literal(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "''"))
}
