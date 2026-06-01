// Search module — currently unused; FTS5 search is in db.rs.
// Pattern/regex conversion kept for potential future CLI use.

use regex::Regex;

/// Convert a wildcard pattern to a regex.
pub fn convert_wildcard_to_regex(pattern: &str, case_sensitive: bool) -> Result<Regex, regex::Error> {
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

    if case_sensitive {
        Regex::new(&format!("^{}$", regex_pattern))
    } else {
        Regex::new(&format!("(?i)^{}$", regex_pattern))
    }
}
