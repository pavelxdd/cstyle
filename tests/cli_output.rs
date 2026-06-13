use std::process::{Command, Stdio};

fn cstyle_binary() -> &'static str {
    env!("CARGO_BIN_EXE_cstyle")
}

fn help_output() -> String {
    let output = Command::new(cstyle_binary())
        .arg("--help")
        .output()
        .expect("run cstyle --help");
    assert!(output.status.success(), "status: {}", output.status);
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("help is utf-8")
}

#[test]
fn closed_stdout_is_reported_without_panicking() {
    let mut child = Command::new(cstyle_binary())
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run cstyle --version");
    drop(child.stdout.take());

    let output = child.wait_with_output().expect("wait for cstyle");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
}

#[test]
fn help_lists_supported_options_and_omits_unsupported_options() {
    let help = help_output();

    for needle in [
        "Utility and meta options",
        "Formatter options",
        "--options=PATH",
        "--error-on-changes",
        "--accept-empty-list",
        "--project=NAME",
        "--mode=c|objc",
        "--style=allman",
        "--style=webkit",
        "-A17",
        "-A1",
        "--indent=spaces=N",
        "-sN",
        "--indent-continuation=N",
        "--max-continuation-indent=N",
        "--min-conditional-indent=N",
        "--indent-switches",
        "--indent-preproc-cond",
        "--pad-oper",
        "--pad-paren-out",
        "--align-pointer=type",
        "--align-reference=none",
        "--break-blocks=all",
        "--no-indent-if-after-else",
        "--attach-return-type-decl",
        "--max-code-length=N",
        "--lineend=linux",
        "--line-between-members=all",
        "--access-label=LABEL",
        "--macro-block=BEGIN:END",
        "--control-header=NAME",
        "--non-paren-header=NAME",
        "--pad-method-prefix",
        "--pad-method-colon=none|all|after|before",
    ] {
        assert!(
            help.contains(needle),
            "missing help entry {needle:?}\n{help}"
        );
    }

    for needle in ["--squeeze-lines", "--indent-lambda", "--completions"] {
        assert!(
            !help.contains(needle),
            "help advertised unimplemented option {needle:?}\n{help}"
        );
    }
}
