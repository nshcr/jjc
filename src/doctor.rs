use std::env;
use std::io;
use std::process::Command;

pub const TESTED_JJ_PROTOCOL_BASELINE: &str = "0.45.1";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum JjCompatibility {
    Tested,
    OlderUntested,
    NewerUntested,
    DevelopmentUntested,
    Unknown,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DoctorReport {
    pub jj_version: Option<String>,
    pub jj_error: Option<String>,
    pub jjc_program: String,
}

impl DoctorReport {
    pub fn ok(&self) -> bool {
        self.jj_version.is_some()
    }

    pub fn text(&self) -> String {
        let mut text = String::from("jjc doctor\n\n");
        match (&self.jj_version, &self.jj_error) {
            (Some(version), _) => match self.compatibility() {
                JjCompatibility::Tested => {
                    text.push_str(&format!("ok jj: {version} (tested protocol)\n"));
                }
                JjCompatibility::OlderUntested => {
                    text.push_str(&format!(
                            "warning jj: {version} is older than tested protocol {TESTED_JJ_PROTOCOL_BASELINE}\n"
                        ));
                }
                JjCompatibility::NewerUntested => {
                    text.push_str(&format!(
                            "warning jj: {version} is newer than tested protocol {TESTED_JJ_PROTOCOL_BASELINE}\n"
                        ));
                }
                JjCompatibility::DevelopmentUntested => {
                    text.push_str(&format!(
                        "warning jj: {version} is a development or prerelease build; tested protocol baseline is {TESTED_JJ_PROTOCOL_BASELINE}\n"
                    ));
                }
                JjCompatibility::Unknown => {
                    text.push_str(&format!(
                            "warning jj: could not compare {version:?} with tested protocol {TESTED_JJ_PROTOCOL_BASELINE}\n"
                        ));
                }
            },
            (None, Some(error)) => {
                text.push_str(&format!("missing jj: {error}\n"));
            }
            (None, None) => {
                text.push_str("missing jj: jj was not found on PATH\n");
            }
        }
        text.push_str(&format!("ok jjc: {}\n\n", self.jjc_program));
        text.push_str(&format!(
            "tested jj protocol baseline: {TESTED_JJ_PROTOCOL_BASELINE}\n\n"
        ));
        text.push_str("recommended jj config:\n");
        text.push_str(&recommended_config(&self.jjc_program));
        text
    }

    fn compatibility(&self) -> JjCompatibility {
        let Some(version) = self.jj_version.as_deref().and_then(parse_jj_version) else {
            return JjCompatibility::Unknown;
        };
        if version.is_prerelease {
            return JjCompatibility::DevelopmentUntested;
        }
        let Some(baseline) = parse_jj_version(TESTED_JJ_PROTOCOL_BASELINE) else {
            return JjCompatibility::Unknown;
        };
        match version.triplet.cmp(&baseline.triplet) {
            std::cmp::Ordering::Less => JjCompatibility::OlderUntested,
            std::cmp::Ordering::Equal => JjCompatibility::Tested,
            std::cmp::Ordering::Greater => JjCompatibility::NewerUntested,
        }
    }
}

pub fn run() -> io::Result<()> {
    let report = DoctorReport::detect();
    println!("{}", report.text());
    if report.ok() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "jj was not found on PATH",
        ))
    }
}

impl DoctorReport {
    fn detect() -> Self {
        let jjc_program = env::current_exe()
            .ok()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "jjc".to_owned());
        match Command::new("jj").arg("--version").output() {
            Ok(output) if output.status.success() => Self {
                jj_version: Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()),
                jj_error: None,
                jjc_program,
            },
            Ok(output) => Self {
                jj_version: None,
                jj_error: Some(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
                jjc_program,
            },
            Err(error) => Self {
                jj_version: None,
                jj_error: Some(error.to_string()),
                jjc_program,
            },
        }
    }
}

fn recommended_config(program: &str) -> String {
    let program = toml_string(program);
    format!(
        "[ui]\n\
         editor = [{program}, \"edit\"]\n\
         diff-editor = \"jjc\"\n\
         merge-editor = \"jjc\"\n\
         \n\
         [merge-tools.jjc]\n\
         program = {program}\n\
         edit-args = [\"diff\", \"$left\", \"$right\", \"$output\"]\n\
         merge-args = [\"merge\", \"$left\", \"$base\", \"$right\", \"$output\", \"--marker-length\", \"$marker_length\", \"--path\", \"$path\"]\n\
         merge-tool-edits-conflict-markers = true\n\
         conflict-marker-style = \"git\"\n"
    )
}

fn toml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[derive(Debug, Eq, PartialEq)]
struct JjVersion {
    triplet: (u64, u64, u64),
    is_prerelease: bool,
}

fn parse_jj_version(value: &str) -> Option<JjVersion> {
    value.split_whitespace().find_map(|part| {
        let part = part.trim_start_matches('v');
        let (version, metadata) = part
            .split_once('+')
            .map_or((part, None), |(version, metadata)| {
                (version, Some(metadata))
            });
        let (version, prerelease) = version
            .split_once('-')
            .map_or((version, None), |(version, prerelease)| {
                (version, Some(prerelease))
            });
        for suffix in [metadata, prerelease].into_iter().flatten() {
            if !suffix.split('.').all(|identifier| {
                !identifier.is_empty()
                    && identifier
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '-')
            }) {
                return None;
            }
        }
        let mut numbers = version.split('.');
        let major = numbers.next()?.parse().ok()?;
        let minor = numbers.next()?.parse().ok()?;
        let patch = numbers.next()?.parse().ok()?;
        if numbers.next().is_some() {
            return None;
        }
        Some(JjVersion {
            triplet: (major, minor, patch),
            is_prerelease: prerelease.is_some(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_config_escapes_program_path() {
        let config = recommended_config(r#"/tmp/a "quoted" path/jjc"#);

        assert!(config.contains(r#"editor = ["/tmp/a \"quoted\" path/jjc", "edit"]"#));
        assert!(config.contains(r#"program = "/tmp/a \"quoted\" path/jjc""#));
    }

    #[test]
    fn recommended_config_prefills_git_conflict_markers() {
        let program = "/tmp/jjc with spaces";
        let config: toml::Value = toml::from_str(&recommended_config(program)).unwrap();
        let ui = &config["ui"];
        let tool = &config["merge-tools"]["jjc"];

        assert_eq!(ui["editor"][0].as_str(), Some(program));
        assert_eq!(ui["editor"][1].as_str(), Some("edit"));
        assert_eq!(ui["diff-editor"].as_str(), Some("jjc"));
        assert_eq!(ui["merge-editor"].as_str(), Some("jjc"));
        assert_eq!(tool["program"].as_str(), Some(program));
        assert_eq!(tool["edit-args"][0].as_str(), Some("diff"));
        assert_eq!(tool["merge-args"][0].as_str(), Some("merge"));
        assert_eq!(
            tool["merge-tool-edits-conflict-markers"].as_bool(),
            Some(true)
        );
        assert_eq!(tool["conflict-marker-style"].as_str(), Some("git"));
    }

    #[test]
    fn missing_jj_report_is_not_ok() {
        let report = DoctorReport {
            jj_version: None,
            jj_error: Some("not found".to_owned()),
            jjc_program: "jjc".to_owned(),
        };

        assert!(!report.ok());
        assert!(report.text().contains("missing jj: not found"));
        assert!(
            report
                .text()
                .contains("tested jj protocol baseline: 0.45.1")
        );
        assert!(report.text().contains("recommended jj config:"));
    }

    #[test]
    fn reports_exact_and_drifted_jj_versions_truthfully() {
        let tested = DoctorReport {
            jj_version: Some("jj 0.45.1".to_owned()),
            jj_error: None,
            jjc_program: "jjc".to_owned(),
        };
        let newer = DoctorReport {
            jj_version: Some("jj 0.46.0".to_owned()),
            jj_error: None,
            jjc_program: "jjc".to_owned(),
        };

        assert_eq!(tested.compatibility(), JjCompatibility::Tested);
        assert!(tested.text().contains("ok jj: jj 0.45.1 (tested protocol)"));
        assert_eq!(newer.compatibility(), JjCompatibility::NewerUntested);
        assert!(newer.text().contains("warning jj:"));
        assert!(!newer.text().contains("ok jj:"));

        let older = DoctorReport {
            jj_version: Some("jj 0.44.0".to_owned()),
            ..tested
        };
        assert_eq!(older.compatibility(), JjCompatibility::OlderUntested);
        assert!(
            older
                .text()
                .contains("is older than tested protocol 0.45.1")
        );
    }

    #[test]
    fn development_and_prerelease_builds_are_never_reported_as_tested() {
        for version in [
            "jj 0.45.1-git",
            "jj 0.45.1-rc.1",
            "jj 0.45.1-git-a1b2c3d4",
            "jj 0.45.1-rc.1+build.7",
            "jj 0.46.0-dev",
            "jj 0.44.0-git",
        ] {
            let report = DoctorReport {
                jj_version: Some(version.to_owned()),
                jj_error: None,
                jjc_program: "jjc".to_owned(),
            };

            assert_eq!(report.compatibility(), JjCompatibility::DevelopmentUntested);
            assert!(report.ok());
            assert!(report.text().contains("development or prerelease build"));
            assert!(!report.text().contains("ok jj:"));
        }
    }

    #[test]
    fn parses_plain_and_decorated_jj_versions() {
        for version in [
            "0.45.1",
            "jj 0.45.1",
            "jj v0.45.1",
            "jj 0.45.1 (a1b2c3d4)",
            "jj 0.45.1+build.7",
        ] {
            assert_eq!(
                parse_jj_version(version),
                Some(JjVersion {
                    triplet: (0, 45, 1),
                    is_prerelease: false,
                })
            );
        }
        for version in [
            "unknown",
            "jj 0.45",
            "jj 0.45.1.2",
            "jj 0.45.1-",
            "jj 0.45.1+",
            "jj 0.45.1-git..1",
            "jj 0.45.1unexpected",
        ] {
            assert_eq!(parse_jj_version(version), None);
        }
    }
}
