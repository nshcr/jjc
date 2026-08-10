use std::ffi::OsStr;
use std::process::Command;
use std::process::Output;

pub fn jj_available() -> bool {
    let output = Command::new("jj").arg("--version").output();
    let Some(output) = available_output("jj --version", output) else {
        return false;
    };

    if let Some(expected) = std::env::var_os("JJC_EXPECT_JJ_VERSION") {
        assert_expected_jj_version(&output, &expected);
    }
    true
}

fn available_output(label: &str, result: std::io::Result<Output>) -> Option<Output> {
    match result {
        Ok(output) if output.status.success() => Some(output),
        Ok(output) => {
            integration_unavailable(format!(
                "{label} exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
            None
        }
        Err(error) => {
            integration_unavailable(format!("failed to run {label}: {error}"));
            None
        }
    }
}

fn integration_unavailable(message: String) {
    if std::env::var_os("JJC_REQUIRE_INTEGRATION").is_some() {
        panic!("integration prerequisite unavailable: {message}");
    }
}

fn assert_expected_jj_version(output: &Output, expected: &OsStr) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual = stdout
        .trim()
        .strip_prefix("jj ")
        .and_then(|version| version.split_whitespace().next())
        .unwrap_or_default();
    assert_eq!(
        actual,
        expected.to_string_lossy(),
        "strict integration must run against the declared jj protocol baseline"
    );
}
