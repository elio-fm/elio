use std::path::Path;

use super::Shell;

pub(crate) fn binary_command(shell: Shell, invocation: Option<&str>, executable: &Path) -> String {
    match shell {
        Shell::Bash | Shell::Zsh | Shell::Fish => posix_binary_command(invocation, executable),
        Shell::Nu => nu_binary_command(invocation, executable),
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

pub(crate) fn init_script(shell: Shell, binary: &str) -> String {
    match shell {
        Shell::Bash | Shell::Zsh => posix_init_script(binary),
        Shell::Fish => fish_init_script(binary),
        Shell::Nu => nu_init_script(binary),
    }
}
fn posix_init_script(executable: &str) -> String {
    format!(
        r#"elio() {{
    case "${{1-}}" in
        shell|-*)
            {executable} "$@"
            return $?
            ;;
    esac

    local tmp cwd status_code
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
        case shell '-*'
            {executable} $argv
            return $status
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
  if (($args | length) > 0) and (
    (($args.0 | into string) == 'shell') or
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

  if ($cwd | is-not-empty) and ($cwd != $env.PWD) and (($cwd | path type) == 'dir') {{
    cd $cwd
  }}

  $env.LAST_EXIT_CODE = $status_code
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
