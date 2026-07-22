use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn launch(
    target: &str,
    provider: &str,
    session_id: &str,
    cwd: Option<&str>,
    source_path: &str,
) -> Result<(), String> {
    match target {
        "chat" => {
            if !session_id_is_safe(session_id) {
                return Err("This chat ID cannot be opened safely".to_string());
            }

            let (program, arguments) = match provider {
                "claude" => ("claude", vec!["--resume", session_id]),
                "codex" => ("codex", vec!["resume", session_id]),
                _ => return Err(format!("Unsupported chat provider: {provider}")),
            };
            launch_terminal(
                program,
                &arguments,
                working_directory(cwd, source_path).as_deref(),
            )
        }
        "vscode" => launch_vscode(working_directory(cwd, source_path).as_deref()),
        _ => Err(format!("Unsupported launch target: {target}")),
    }
}

fn session_id_is_safe(session_id: &str) -> bool {
    !session_id.is_empty()
        && session_id.len() <= 160
        && session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn working_directory(cwd: Option<&str>, source_path: &str) -> Option<PathBuf> {
    cwd.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| {
            Path::new(source_path)
                .parent()
                .filter(|path| path.is_dir())
                .map(Path::to_path_buf)
        })
}

#[cfg(target_os = "macos")]
fn launch_terminal(program: &str, arguments: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    let shell_command = terminal_shell_command(program, arguments, cwd);
    let script = format!(
        "tell application \"Terminal\" to do script \"{}\"",
        escape_applescript(&shell_command)
    );
    Command::new("/usr/bin/osascript")
        .args([
            "-e",
            &script,
            "-e",
            "tell application \"Terminal\" to activate",
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Terminal: {error}"))
}

#[cfg(target_os = "windows")]
fn launch_terminal(program: &str, arguments: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    let mut command = Command::new("cmd.exe");
    command.arg("/K").arg(program).args(arguments);
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }
    command.creation_flags(CREATE_NEW_CONSOLE);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Command Prompt: {error}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn launch_terminal(program: &str, arguments: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    let terminals: [(&str, &[&str]); 6] = [
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("kitty", &[]),
        ("alacritty", &["-e"]),
        ("wezterm", &["start", "--"]),
    ];

    for (terminal, prefix) in terminals {
        let mut command = Command::new(terminal);
        command.args(prefix).arg(program).args(arguments);
        if let Some(directory) = cwd {
            command.current_dir(directory);
        }
        match command.spawn() {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("Could not open {terminal}: {error}")),
        }
    }

    Err("No supported terminal emulator was found".to_string())
}

#[cfg(target_os = "macos")]
fn launch_vscode(target: Option<&Path>) -> Result<(), String> {
    let mut command = Command::new("/usr/bin/open");
    command.args(["-a", "Visual Studio Code"]);
    if let Some(path) = target {
        command.arg(path);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Visual Studio Code: {error}"))
}

#[cfg(target_os = "windows")]
fn launch_vscode(target: Option<&Path>) -> Result<(), String> {
    let executable = windows_vscode_executable()
        .ok_or_else(|| "Visual Studio Code could not be found".to_string())?;
    let mut command = Command::new(executable);
    command.arg("--reuse-window");
    if let Some(path) = target {
        command.arg(path);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Visual Studio Code: {error}"))
}

#[cfg(target_os = "windows")]
fn windows_vscode_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Programs")
                .join("Microsoft VS Code")
                .join("Code.exe"),
        );
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(program_files) = std::env::var_os(variable) {
            candidates.push(
                PathBuf::from(program_files)
                    .join("Microsoft VS Code")
                    .join("Code.exe"),
            );
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            candidates.push(directory.join("Code.exe"));
            if let Some(parent) = directory.parent() {
                candidates.push(parent.join("Code.exe"));
            }
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn launch_vscode(target: Option<&Path>) -> Result<(), String> {
    for executable in ["code", "code-insiders"] {
        let mut command = Command::new(executable);
        command.arg("--reuse-window");
        if let Some(path) = target {
            command.arg(path);
        }
        match command.spawn() {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("Could not open Visual Studio Code: {error}")),
        }
    }
    Err("Visual Studio Code CLI could not be found".to_string())
}

#[cfg(any(target_os = "macos", test))]
fn terminal_shell_command(program: &str, arguments: &[&str], cwd: Option<&Path>) -> String {
    let invocation = std::iter::once(program)
        .chain(arguments.iter().copied())
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ");
    match cwd {
        Some(directory) => format!(
            "cd -- {} && exec {invocation}",
            shell_quote(&directory.to_string_lossy())
        ),
        None => format!("exec {invocation}"),
    }
}

#[cfg(any(target_os = "macos", test))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(any(target_os = "macos", test))]
fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_session_ids() {
        assert!(session_id_is_safe("91503f92-e0e9-4535-858e-327f37d18ff7"));
        assert!(session_id_is_safe("rollout_2026.07.22"));
        assert!(!session_id_is_safe("bad; open evil"));
        assert!(!session_id_is_safe(""));
    }

    #[test]
    fn quotes_terminal_commands() {
        let directory = Path::new("/tmp/a user's project");
        let command = terminal_shell_command("claude", &["--resume", "chat-id"], Some(directory));
        assert_eq!(
            command,
            "cd -- '/tmp/a user'\"'\"'s project' && exec 'claude' '--resume' 'chat-id'"
        );
        assert_eq!(
            escape_applescript("a \\\"quote\\\""),
            "a \\\\\\\"quote\\\\\\\""
        );
    }
}
