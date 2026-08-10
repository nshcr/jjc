use std::env;
use std::io;
use std::process::Command;

pub const TESTED_JJ_PROTOCOL_BASELINE: &str = "0.44.0";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum JjCompatibility {
    Tested,
    OlderUntested,
    NewerUntested,
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
        let Some(version) = self.jj_version.as_deref().and_then(version_triplet) else {
            return JjCompatibility::Unknown;
        };
        let Some(baseline) = version_triplet(TESTED_JJ_PROTOCOL_BASELINE) else {
            return JjCompatibility::Unknown;
        };
        match version.cmp(&baseline) {
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

fn version_triplet(value: &str) -> Option<(u64, u64, u64)> {
    value.split_whitespace().find_map(|part| {
        let part = part.trim_start_matches('v');
        let mut numbers = part.split('.');
        let major = numbers.next()?.parse().ok()?;
        let minor = numbers.next()?.parse().ok()?;
        let patch = numbers
            .next()?
            .split(|character: char| !character.is_ascii_digit())
            .next()?
            .parse()
            .ok()?;
        Some((major, minor, patch))
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
                .contains("tested jj protocol baseline: 0.44.0")
        );
        assert!(report.text().contains("recommended jj config:"));
    }

    #[test]
    fn reports_exact_and_drifted_jj_versions_truthfully() {
        let tested = DoctorReport {
            jj_version: Some("jj 0.44.0".to_owned()),
            jj_error: None,
            jjc_program: "jjc".to_owned(),
        };
        let newer = DoctorReport {
            jj_version: Some("jj 0.45.1".to_owned()),
            jj_error: None,
            jjc_program: "jjc".to_owned(),
        };

        assert_eq!(tested.compatibility(), JjCompatibility::Tested);
        assert!(tested.text().contains("ok jj: jj 0.44.0 (tested protocol)"));
        assert_eq!(newer.compatibility(), JjCompatibility::NewerUntested);
        assert!(newer.text().contains("warning jj:"));
        assert!(!newer.text().contains("ok jj:"));
    }

    #[test]
    fn parses_plain_and_decorated_jj_versions() {
        assert_eq!(version_triplet("0.44.0"), Some((0, 44, 0)));
        assert_eq!(version_triplet("jj 0.44.0"), Some((0, 44, 0)));
        assert_eq!(version_triplet("jj 0.44.0-git"), Some((0, 44, 0)));
        assert_eq!(version_triplet("unknown"), None);
    }
}
