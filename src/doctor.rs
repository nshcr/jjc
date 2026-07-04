use std::env;
use std::io;
use std::process::Command;

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
            (Some(version), _) => {
                text.push_str(&format!("ok jj: {version}\n"));
            }
            (None, Some(error)) => {
                text.push_str(&format!("missing jj: {error}\n"));
            }
            (None, None) => {
                text.push_str("missing jj: jj was not found on PATH\n");
            }
        }
        text.push_str(&format!("ok jjc: {}\n\n", self.jjc_program));
        text.push_str("recommended jj config:\n");
        text.push_str(&recommended_config(&self.jjc_program));
        text
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
         merge-args = [\"merge\", \"$left\", \"$base\", \"$right\", \"$output\", \"--marker-length\", \"$marker_length\", \"--path\", \"$path\"]\n"
    )
}

fn toml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
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
    fn missing_jj_report_is_not_ok() {
        let report = DoctorReport {
            jj_version: None,
            jj_error: Some("not found".to_owned()),
            jjc_program: "jjc".to_owned(),
        };

        assert!(!report.ok());
        assert!(report.text().contains("missing jj: not found"));
        assert!(report.text().contains("recommended jj config:"));
    }
}
