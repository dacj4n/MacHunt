use regex::Regex;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct ExcludeRules {
    exact_dirs: Vec<PathBuf>,
    regex_dirs: Vec<Regex>,
}

impl ExcludeRules {
    pub fn empty() -> Self {
        Self {
            exact_dirs: Vec::new(),
            regex_dirs: Vec::new(),
        }
    }
}

pub fn sanitize_rules(values: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() {
            continue;
        }
        if out.iter().any(|existing| existing == normalized) {
            continue;
        }
        out.push(normalized.to_string());
    }
    out
}

pub fn sanitize_owned_rules(values: Vec<String>) -> Vec<String> {
    sanitize_rules(&values)
}

pub fn sanitize_roots(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = if trimmed == "/" {
            "/".to_string()
        } else {
            trimmed.trim_end_matches('/').to_string()
        };
        if !normalized.starts_with('/') {
            continue;
        }
        if out.iter().any(|existing| existing == &normalized) {
            continue;
        }
        out.push(normalized);
    }
    out
}

pub fn wildcard_to_regex(pattern: &str) -> Result<Regex, regex::Error> {
    let mut regex_pattern = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    regex_pattern.push_str(".*");
                    i += 2;
                } else {
                    regex_pattern.push_str("[^/]*");
                    i += 1;
                }
            }
            '?' => {
                regex_pattern.push_str("[^/]");
                i += 1;
            }
            c => {
                regex_pattern.push_str(&regex::escape(&c.to_string()));
                i += 1;
            }
        }
    }

    Regex::new(&format!("(?i)^{}$", regex_pattern))
}

pub fn compile_pattern(pattern: &str) -> Result<Regex, String> {
    // Try as raw regex first. If it parses, wrap with (?i) for case-insensitive matching
    // (consistent with wildcard_to_regex behavior and macOS case-insensitive filesystem).
    Regex::new(&format!("(?i){}", pattern))
        .or_else(|_| wildcard_to_regex(pattern))
        .map_err(|err| err.to_string())
}

pub fn validate_pattern_rules(patterns: &[String]) -> Result<(), String> {
    for pattern in patterns {
        compile_pattern(pattern)
            .map_err(|err| format!("Invalid pattern '{}': {}", pattern, err))?;
    }
    Ok(())
}

pub fn compile_exclude_rules(exact_dirs: &[String], regex_dirs: &[String]) -> ExcludeRules {
    let exact_dirs = sanitize_rules(exact_dirs)
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    let mut compiled_regex = Vec::new();
    for raw in sanitize_rules(regex_dirs) {
        if let Ok(re) = compile_pattern(&raw) {
            compiled_regex.push(re);
        }
    }

    ExcludeRules {
        exact_dirs,
        regex_dirs: compiled_regex,
    }
}

fn to_matchable_path(path: &Path, is_dir: bool) -> String {
    let mut s = path.to_string_lossy().to_string();
    if is_dir && !s.ends_with('/') {
        s.push('/');
    }
    s
}

pub fn is_excluded(path: &Path, is_dir: bool, rules: &ExcludeRules) -> bool {
    if rules.exact_dirs.iter().any(|dir| path.starts_with(dir)) {
        return true;
    }

    if rules.regex_dirs.is_empty() {
        return false;
    }

    let path_text = to_matchable_path(path, is_dir);
    rules.regex_dirs.iter().any(|re| re.is_match(&path_text))
}

// ── File-level exclusion (dot files, extension patterns) ──

#[derive(Clone, Debug)]
pub struct FileExcludeRules {
    pub exclude_dot_files: bool,
    pub file_patterns: Vec<Regex>,
}

impl FileExcludeRules {
    pub fn empty() -> Self {
        Self {
            exclude_dot_files: false,
            file_patterns: Vec::new(),
        }
    }
}

pub fn compile_file_exclude_rules(
    exclude_dot_files: bool,
    file_patterns: &[String],
) -> FileExcludeRules {
    let sanitized = sanitize_rules(file_patterns);
    let mut compiled = Vec::new();
    for raw in &sanitized {
        if let Ok(re) = compile_pattern(raw) {
            compiled.push(re);
        }
    }
    FileExcludeRules {
        exclude_dot_files,
        file_patterns: compiled,
    }
}

/// Check whether a file should be excluded by file-level rules.
/// Only checks file name (not full path). Returns false for directories
/// (directory exclusion is handled by `is_excluded`).
pub fn is_file_excluded(path: &Path, is_dir: bool, rules: &FileExcludeRules) -> bool {
    // File-level exclusion only applies to files, not directories.
    // Directories are handled by the directory-level ExcludeRules.
    if is_dir {
        return false;
    }
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => return false,
    };
    // Check dot files
    if rules.exclude_dot_files && file_name.starts_with('.') {
        return true;
    }
    // Check file name patterns
    if !rules.file_patterns.is_empty() {
        let name_lower = file_name.to_lowercase();
        if rules
            .file_patterns
            .iter()
            .any(|re| re.is_match(&name_lower))
        {
            return true;
        }
    }
    false
}

/// Check whether a directory is a dot-directory (for filter_entry optimization).
pub fn is_dot_dir(path: &Path, is_dir: bool) -> bool {
    if !is_dir {
        return false;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}

