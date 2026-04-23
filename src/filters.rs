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
                    regex_pattern.push_str(".*");
                    i += 1;
                }
            }
            '?' => {
                regex_pattern.push('.');
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
    Regex::new(pattern)
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

