use crate::model::{SearchMode, SearchOptions};
use dashmap::DashMap;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

pub fn search(index: &Arc<DashMap<String, Vec<PathBuf>>>, options: SearchOptions) -> Vec<PathBuf> {
    let options = options.normalize();
    match options.mode {
        SearchMode::Substring => search_substring(index, &options),
        SearchMode::Pattern => search_pattern(index, &options),
    }
}

fn path_allowed(path: &Path, include_files: bool, include_dirs: bool) -> bool {
    (include_files && path.is_file()) || (include_dirs && path.is_dir())
}

fn prefix_allowed(path: &Path, prefix: &Option<PathBuf>) -> bool {
    match prefix {
        Some(p) => path.starts_with(p),
        None => true,
    }
}

fn search_substring(
    index: &Arc<DashMap<String, Vec<PathBuf>>>,
    options: &SearchOptions,
) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let query_lower = options.query.to_lowercase();

    for r in index.iter() {
        let (file_name, paths) = r.pair();
        if !file_name.contains(&query_lower) {
            continue;
        }
        for path in paths.iter() {
            if !path_allowed(path.as_path(), options.include_files, options.include_dirs) {
                continue;
            }
            if !prefix_allowed(path.as_path(), &options.path_prefix) {
                continue;
            }
            results.push(path.clone());
        }
    }

    results
}

fn search_pattern(
    index: &Arc<DashMap<String, Vec<PathBuf>>>,
    options: &SearchOptions,
) -> Vec<PathBuf> {
    let mut results = Vec::new();

    let regex = match convert_wildcard_to_regex(&options.query) {
        Ok(re) => re,
        Err(_) => return results,
    };

    for r in index.iter() {
        let (file_name, paths) = r.pair();
        if !regex.is_match(file_name) {
            continue;
        }
        for path in paths.iter() {
            if !path_allowed(path.as_path(), options.include_files, options.include_dirs) {
                continue;
            }
            if !prefix_allowed(path.as_path(), &options.path_prefix) {
                continue;
            }
            results.push(path.clone());
        }
    }

    results
}

fn convert_wildcard_to_regex(pattern: &str) -> Result<Regex, regex::Error> {
    let mut regex_pattern = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

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
            '{' => {
                regex_pattern.push('(');
                i += 1;
            }
            '}' => {
                regex_pattern.push(')');
                i += 1;
            }
            ',' => {
                regex_pattern.push('|');
                i += 1;
            }
            c => {
                regex_pattern.push_str(&regex::escape(&c.to_string()));
                i += 1;
            }
        }
    }

    Regex::new(&format!("^{}$", regex_pattern))
}
