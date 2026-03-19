use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum Command {
    Verbose,
    Describe(String),
    Launch(String),
    Inspect,
    Click(String, String),
    Fill(String, String),
}

#[derive(Debug)]
pub struct TestFile {
    pub path: String,
    pub commands: Vec<Command>,
}

impl TestFile {
    pub fn is_verbose(&self) -> bool {
        self.commands.iter().any(|c| matches!(c, Command::Verbose))
    }

    pub fn description(&self) -> Option<String> {
        self.commands.iter().find_map(|c| {
            if let Command::Describe(desc) = c {
                Some(desc.clone())
            } else {
                None
            }
        })
    }
}

pub fn parse_file(path: &Path) -> Result<TestFile, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut commands = Vec::new();

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        let cmd = parse_line(line).ok_or_else(|| {
            format!(
                "{}:{}: syntax error: {}",
                path.display(),
                line_num + 1,
                line
            )
        })?;
        commands.push(cmd);
    }

    Ok(TestFile {
        path: path.display().to_string(),
        commands,
    })
}

fn parse_line(line: &str) -> Option<Command> {
    if line == "verbose()" {
        return Some(Command::Verbose);
    }

    if line == "inspect()" {
        return Some(Command::Inspect);
    }

    if let Some(inner) = extract_single_arg(line, "describe") {
        return Some(Command::Describe(inner));
    }

    if let Some(inner) = extract_single_arg(line, "launch") {
        return Some(Command::Launch(inner));
    }

    if let Some((a, b)) = extract_two_args(line, "click") {
        return Some(Command::Click(a, b));
    }

    if let Some((a, b)) = extract_two_args(line, "fill") {
        return Some(Command::Fill(a, b));
    }

    None
}

fn extract_single_arg(line: &str, func: &str) -> Option<String> {
    let prefix = format!("{}(", func);
    if !line.starts_with(&prefix) || !line.ends_with(')') {
        return None;
    }
    let inner = &line[prefix.len()..line.len() - 1];
    Some(unquote(inner.trim()))
}

fn extract_two_args(line: &str, func: &str) -> Option<(String, String)> {
    let prefix = format!("{}(", func);
    if !line.starts_with(&prefix) || !line.ends_with(')') {
        return None;
    }
    let inner = &line[prefix.len()..line.len() - 1];

    let parts: Vec<&str> = inner.splitn(2, ',').collect();
    if parts.len() != 2 {
        return None;
    }

    Some((unquote(parts[0].trim()), unquote(parts[1].trim())))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
