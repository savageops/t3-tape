use crate::exit::RedtapeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedDiff {
    pub raw: String,
    pub files: Vec<DiffFile>,
}

impl UnifiedDiff {
    pub fn parse(content: &str) -> Result<Self, RedtapeError> {
        let raw = content.replace("\r\n", "\n");
        if raw.trim().is_empty() {
            return Err(RedtapeError::Usage(
                "diff is empty; nothing to record".to_string(),
            ));
        }

        let files = if raw.contains("diff --git ") {
            parse_git_diff(&raw)?
        } else {
            vec![parse_plain_diff(&raw)?]
        };

        if files.is_empty() {
            return Err(RedtapeError::Usage(
                "diff did not contain any file changes".to_string(),
            ));
        }

        Ok(Self { raw, files })
    }

    pub fn changed_paths(&self) -> Vec<String> {
        self.files.iter().map(|file| file.path.clone()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    pub raw: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub header: String,
    pub preimage_lines: Vec<String>,
}

fn parse_git_diff(raw: &str) -> Result<Vec<DiffFile>, RedtapeError> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();

    for line in raw.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            blocks.push(parse_file_block(&join_lines(&current))?);
            current.clear();
        }

        if !current.is_empty() || line.starts_with("diff --git ") {
            current.push(line.to_string());
        }
    }

    if !current.is_empty() {
        blocks.push(parse_file_block(&join_lines(&current))?);
    }

    Ok(blocks)
}

fn parse_plain_diff(raw: &str) -> Result<DiffFile, RedtapeError> {
    parse_file_block(raw)
}

fn parse_file_block(raw: &str) -> Result<DiffFile, RedtapeError> {
    let lines: Vec<&str> = raw.lines().collect();
    let path = determine_path(&lines).ok_or_else(|| {
        RedtapeError::Validation("unable to determine changed file path from diff".to_string())
    })?;

    let mut hunks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if line.starts_with("@@") {
            let header = line.to_string();
            let mut preimage_lines = Vec::new();
            index += 1;
            while index < lines.len() && !lines[index].starts_with("@@") {
                let body_line = lines[index];
                if body_line.starts_with(' ') || body_line.starts_with('-') {
                    preimage_lines.push(body_line.to_string());
                }
                index += 1;
            }

            hunks.push(DiffHunk {
                header,
                preimage_lines,
            });
            continue;
        }

        index += 1;
    }

    let raw = ensure_trailing_newline(raw);
    Ok(DiffFile { path, raw, hunks })
}

fn determine_path(lines: &[&str]) -> Option<String> {
    for line in lines {
        if let Some(path) = line.strip_prefix("+++ ") {
            if path != "/dev/null" {
                return Some(normalize_diff_path(path));
            }
        }
    }

    for line in lines {
        if let Some(path) = line.strip_prefix("--- ") {
            if path != "/dev/null" {
                return Some(normalize_diff_path(path));
            }
        }
    }

    let header = lines.first()?;
    let mut parts = header.split_whitespace();
    let _ = parts.next();
    let _ = parts.next();
    let _ = parts.next();
    let rhs = parts.next()?;
    Some(normalize_diff_path(rhs))
}

fn normalize_diff_path(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .to_string()
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

fn join_lines(lines: &[String]) -> String {
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}
