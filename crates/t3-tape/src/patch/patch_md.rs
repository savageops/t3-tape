use crate::exit::RedtapeError;

use super::patch_id::PatchId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchDocument {
    pub header: String,
    pub entries: Vec<PatchEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchHeader {
    pub project: String,
    pub upstream: String,
    pub base_ref: String,
    pub protocol: String,
    pub state_root: Option<String>,
}

impl PatchDocument {
    pub fn find(&self, id: PatchId) -> Option<&PatchEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchEntry {
    pub id: PatchId,
    pub title: String,
    pub status: String,
    pub surface: String,
    pub added: String,
    pub author: String,
    pub intent: String,
    pub behavior_assertions: Vec<String>,
    pub scope_files: Vec<String>,
    pub scope_components: Vec<String>,
    pub scope_entry_points: Vec<String>,
    pub requires: Vec<String>,
    pub conflicts_with: Vec<String>,
    pub notes: Option<String>,
    pub extra_sections: Vec<MarkdownSection>,
    pub raw_block: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownSection {
    pub title: String,
    pub body: String,
}

impl PatchEntry {
    pub fn render_block(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str(&format!("## [{}] {}\n\n", self.id, self.title));
        rendered.push_str(&format!("**status:** {}  \n", self.status));
        rendered.push_str(&format!("**surface:** {}  \n", self.surface));
        rendered.push_str(&format!("**added:** {}  \n", self.added));
        rendered.push_str(&format!("**author:** {}  \n", self.author));

        rendered.push_str("\n### Intent\n\n");
        rendered.push_str(self.intent.trim());
        rendered.push_str("\n\n### Behavior Contract\n\n");
        if self.behavior_assertions.is_empty() {
            rendered.push('\n');
        } else {
            for assertion in &self.behavior_assertions {
                rendered.push_str(&format!("- {assertion}\n"));
            }
        }

        rendered.push_str("\n### Scope\n\n");
        rendered.push_str(&format!(
            "- **files:** {}\n",
            render_list(&self.scope_files)
        ));
        rendered.push_str(&format!(
            "- **components:** {}\n",
            render_list(&self.scope_components)
        ));
        rendered.push_str(&format!(
            "- **entry-points:** {}\n",
            render_list(&self.scope_entry_points)
        ));

        rendered.push_str("\n### Dependencies\n\n");
        rendered.push_str(&format!(
            "- **requires:** {}\n",
            render_list(&self.requires)
        ));
        rendered.push_str(&format!(
            "- **conflicts-with:** {}\n",
            render_list(&self.conflicts_with)
        ));

        for section in &self.extra_sections {
            rendered.push_str(&format!("\n### {}\n\n", section.title));
            rendered.push_str(section.body.trim_end());
            rendered.push('\n');
        }

        if let Some(notes) = &self.notes {
            rendered.push_str("\n### Notes\n\n");
            rendered.push_str(notes.trim());
            rendered.push('\n');
        }

        rendered.push_str("\n---\n");
        rendered
    }
}

pub fn parse(content: &str) -> Result<PatchDocument, RedtapeError> {
    let normalized = content.replace("\r\n", "\n");
    let starts = block_starts(&normalized);

    if starts.is_empty() {
        return Ok(PatchDocument {
            header: ensure_trailing_newline(&normalized),
            entries: Vec::new(),
        });
    }

    let header = ensure_trailing_newline(&normalized[..starts[0]]);
    let mut entries = Vec::new();

    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(normalized.len());
        let raw_block = ensure_trailing_newline(normalized[*start..end].trim_end_matches('\n'));
        entries.push(parse_block(&raw_block)?);
    }

    Ok(PatchDocument { header, entries })
}

pub fn append_entries(existing: &str, entries: &[PatchEntry]) -> String {
    let mut rendered = ensure_trailing_newline(existing);
    for entry in entries {
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str(&entry.render_block());
    }
    rendered
}

pub fn parse_header(header: &str) -> Result<PatchHeader, RedtapeError> {
    let mut project = None;
    let mut upstream = None;
    let mut base_ref = None;
    let mut protocol = None;
    let mut state_root = None;

    for line in header.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("> project:") {
            project = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("> upstream:") {
            upstream = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("> base-ref:") {
            base_ref = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("> protocol:") {
            protocol = Some(value.trim().to_string());
        } else if let Some(value) = trimmed.strip_prefix("> state-root:") {
            state_root = Some(value.trim().to_string());
        }
    }

    Ok(PatchHeader {
        project: project.ok_or_else(|| missing_header_field("project"))?,
        upstream: upstream.ok_or_else(|| missing_header_field("upstream"))?,
        base_ref: base_ref.ok_or_else(|| missing_header_field("base-ref"))?,
        protocol: protocol.ok_or_else(|| missing_header_field("protocol"))?,
        state_root,
    })
}

fn parse_block(raw_block: &str) -> Result<PatchEntry, RedtapeError> {
    let mut lines: Vec<&str> = raw_block.lines().collect();
    while matches!(lines.last(), Some(line) if line.trim() == "---") {
        lines.pop();
    }

    let header_line = lines
        .first()
        .ok_or_else(|| RedtapeError::Validation("patch block was empty".to_string()))?;

    let (id, title) = parse_block_header(header_line)?;

    let mut status = None;
    let mut surface = None;
    let mut added = None;
    let mut author = None;

    let mut sections = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_body = Vec::new();

    for line in lines.iter().skip(1) {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("**status:**") {
            status = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("**surface:**") {
            surface = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("**added:**") {
            added = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("**author:**") {
            author = Some(value.trim().to_string());
            continue;
        }

        if let Some(section_title) = line.strip_prefix("### ") {
            if let Some(previous_title) = current_title.replace(section_title.trim().to_string()) {
                sections.push((previous_title, trim_section_body(&current_body)));
                current_body.clear();
            }
            continue;
        }

        if current_title.is_some() {
            current_body.push((*line).to_string());
        }
    }

    if let Some(previous_title) = current_title {
        sections.push((previous_title, trim_section_body(&current_body)));
    }

    let mut intent = None;
    let mut behavior_assertions = None;
    let mut scope_files = None;
    let mut scope_components = Some(Vec::new());
    let mut scope_entry_points = Some(Vec::new());
    let mut requires = Some(Vec::new());
    let mut conflicts_with = Some(Vec::new());
    let mut notes = None;
    let mut extra_sections = Vec::new();

    for (title, body) in sections {
        match title.as_str() {
            "Intent" => intent = Some(body),
            "Behavior Contract" => behavior_assertions = Some(parse_bullets(&body)),
            "Scope" => {
                scope_files = Some(parse_named_list(&body, "files"));
                scope_components = Some(parse_named_list(&body, "components"));
                scope_entry_points = Some(parse_named_list(&body, "entry-points"));
            }
            "Dependencies" => {
                requires = Some(parse_named_list(&body, "requires"));
                conflicts_with = Some(parse_named_list(&body, "conflicts-with"));
            }
            "Notes" => notes = Some(body),
            _ => extra_sections.push(MarkdownSection { title, body }),
        }
    }

    Ok(PatchEntry {
        id,
        title,
        status: status.ok_or_else(|| missing_field("status"))?,
        surface: surface.ok_or_else(|| missing_field("surface"))?,
        added: added.ok_or_else(|| missing_field("added"))?,
        author: author.ok_or_else(|| missing_field("author"))?,
        intent: intent.ok_or_else(|| missing_section("Intent"))?,
        behavior_assertions: behavior_assertions
            .ok_or_else(|| missing_section("Behavior Contract"))?,
        scope_files: scope_files.ok_or_else(|| missing_section("Scope"))?,
        scope_components: scope_components.unwrap_or_default(),
        scope_entry_points: scope_entry_points.unwrap_or_default(),
        requires: requires.unwrap_or_default(),
        conflicts_with: conflicts_with.unwrap_or_default(),
        notes,
        extra_sections,
        raw_block: raw_block.to_string(),
    })
}

fn parse_block_header(line: &str) -> Result<(PatchId, String), RedtapeError> {
    let remainder = line
        .strip_prefix("## [")
        .ok_or_else(|| RedtapeError::Validation(format!("invalid patch block header: {line}")))?;
    let end = remainder
        .find(']')
        .ok_or_else(|| RedtapeError::Validation(format!("invalid patch block header: {line}")))?;

    let id: PatchId = remainder[..end].parse()?;
    let title = remainder[end + 1..].trim().to_string();
    if title.is_empty() {
        return Err(RedtapeError::Validation(
            "patch block title cannot be empty".to_string(),
        ));
    }

    Ok((id, title))
}

fn parse_bullets(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| line.trim().strip_prefix("- ").map(str::to_string))
        .collect()
}

fn parse_named_list(body: &str, label: &str) -> Vec<String> {
    let prefix = format!("- **{label}:**");
    for line in body.lines() {
        if let Some(value) = line.trim().strip_prefix(&prefix) {
            return parse_inline_list(value.trim());
        }
    }
    Vec::new()
}

fn parse_inline_list(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    let inner = trimmed.trim_start_matches('[').trim_end_matches(']').trim();

    if inner.is_empty() {
        Vec::new()
    } else {
        inner
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn render_list(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", values.join(", "))
    }
}

fn trim_section_body(lines: &[String]) -> String {
    let joined = lines.join("\n");
    joined.trim_matches('\n').to_string()
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

fn block_starts(content: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in content.split_inclusive('\n') {
        if line.starts_with("## [") {
            starts.push(offset);
        }
        offset += line.len();
    }
    if !content.ends_with('\n') {
        let last_line = content
            .rsplit_once('\n')
            .map(|(_, tail)| tail)
            .unwrap_or(content);
        if last_line.starts_with("## [") {
            starts.push(content.len() - last_line.len());
        }
    }
    starts.sort_unstable();
    starts.dedup();
    starts
}

fn missing_field(field: &str) -> RedtapeError {
    RedtapeError::Validation(format!("missing required patch field: {field}"))
}

fn missing_section(section: &str) -> RedtapeError {
    RedtapeError::Validation(format!("missing required patch section: {section}"))
}

fn missing_header_field(field: &str) -> RedtapeError {
    RedtapeError::Validation(format!("missing required patch header field: {field}"))
}

pub fn render_document(document: &PatchDocument) -> String {
    let mut rendered = ensure_trailing_newline(&document.header);
    for entry in &document.entries {
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str(&entry.render_block());
    }
    rendered
}

pub fn rewrite_header_base_ref(header: &str, new_base_ref: &str) -> String {
    let mut lines = Vec::new();
    let mut replaced = false;

    for line in header.lines() {
        if line.starts_with("> base-ref:") {
            lines.push(format!("> base-ref: {new_base_ref}"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if !replaced {
        lines.push(format!("> base-ref: {new_base_ref}"));
    }

    ensure_trailing_newline(&lines.join("\n"))
}
