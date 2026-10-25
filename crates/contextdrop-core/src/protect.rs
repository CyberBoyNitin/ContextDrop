//! Deterministic preservation rules.
//!
//! These rules intentionally favour retaining evidence over maximizing
//! compression. They are heuristics, not a proof that output is semantically
//! equivalent. User-supplied patterns are literal, case-insensitive substrings;
//! they are never evaluated as regular expressions or shell syntax.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::text::split_content_lines;

struct Rule {
    id: &'static str,
    label: &'static str,
    regex: Regex,
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let specs: [(&str, &str, &str); 10] = [
            (
                "errors-and-warnings",
                "error, fatal, warning, or failure marker",
                r"(?i)\b(?:error|errors|fatal|panic|panicked|exception|failed|failing|failure|traceback|segfault|segmentation fault|denied|unauthorized|forbidden|vulnerab\w*|warning|warn)\b|\bassert(?:ion)?(?:\s+failed)?\b|\bcve-\d{4}-\d+\b|错误|錯誤|エラー|ошибка|erreur|fehler|fallo|erro",
            ),
            (
                "stack-trace",
                "stack-trace frame or traceback",
                r#"(?i)^\s*(?:at\s+\S.*|File\s+".+",\s+line\s+\d+.*|Traceback\s+\(most recent call last\):|thread\s+'.+'\s+panicked(?:\s+at\s+.*)?|goroutine\s+\d+\s+\[|Caused by:|\.{3}\s+\d+\s+more|\s*\|\s*[\^~]+.*)$"#,
            ),
            (
                "file-and-line",
                "file path with line number",
                r"(?i)(?:[a-z]:[\\/]|\.{0,2}[\\/])?(?:[\w.@+~\-]+[\\/])*[\w.@+~\-]+\.(?:rs|ts|tsx|js|jsx|mjs|cjs|py|go|java|kt|kts|rb|php|cs|cpp|cc|cxx|c|h|hpp|swift|scala|sh|bash|zsh|ps1|json|jsonl|toml|ya?ml|md|sql|log|txt|cfg|ini|html|css|scss|vue|svelte|proto|gradle|lock):\d+(?::\d+)?",
            ),
            (
                "python-file-and-line",
                "Python file and line reference",
                r#"(?i)File\s+"[^"]+",\s+line\s+\d+"#,
            ),
            (
                "exit-code",
                "exit code or process status",
                r"(?i)\b(?:exit\s+code|exit\s+status|process\s+exited|exited\s+with|terminated\s+with|command\s+failed\s+with\s+exit)\b",
            ),
            (
                "security",
                "security or credential signal",
                r"(?i)\b(?:security|vulnerab\w*|cve-\d{4}-\d+|exploit|malware|ransomware|xss|csrf|sql injection|credential(?:s)?|secret|leak|access denied|permission denied|authentication failed|severity\s*[:=]\s*(?:critical|high)|critical\s+severity)\b",
            ),
            (
                "assertion-detail",
                "assertion or failure-detail line",
                r"(?i)^\s*(?:Expected\b|Received\b|Actual\b|AssertionError\b|Difference\b|Snapshot\b|E\s{3,}\S|>\s+\S|●|at\s+\S+\(.+:\d+:\d+\))",
            ),
            (
                "test-failure",
                "failed test marker",
                r"(?i)(?:^\s*(?:FAILED|FAIL|not ok)\b|\b[1-9]\d*\s+(?:tests?\s+)?failed\b|\b(?:test|tests|suite)\s+failed\b|[✗✘❌])",
            ),
            (
                "summary",
                "test or build summary",
                r"(?i)(?:^\s*(?:test result|test summary|test\s+suites?|tests?:|summary|results?)\b.*\b(?:passed|failed|ignored|skipped|warnings?|errors?|ok|success|failure)\b|\b\d+\s+(?:passed|failed|ignored|skipped|warnings?|errors?)\b|^\s*(?:build|compilation)\s+(?:succeeded|failed|complete|finished)\b)",
            ),
            (
                "http-error",
                "HTTP error response",
                r#"(?i)"(?:GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS)\s[^"]*"\s+(?:4\d{2}|5\d{2})\b|\b(?:status|status_code|statuscode)\s*[:=]\s*(?:4\d{2}|5\d{2})\b"#,
            ),
        ];
        specs
            .into_iter()
            .map(|(id, label, pattern)| Rule {
                id,
                label,
                regex: Regex::new(pattern).expect("built-in protection regex must compile"),
            })
            .collect()
    })
}

#[derive(Debug, Clone)]
pub(crate) struct LineProtection {
    pub protected: bool,
    pub reason_ids: Vec<&'static str>,
    pub user_pattern: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Protector {
    user_patterns: Vec<(String, String)>, // original, lowercase
}

impl Protector {
    pub(crate) fn new(patterns: &[String]) -> Self {
        Self {
            user_patterns: patterns
                .iter()
                .map(|p| (p.clone(), p.to_lowercase()))
                .collect(),
        }
    }

    /// Direct (line-local) protection rules.
    pub(crate) fn inspect_line(&self, content: &str, _is_first_line: bool) -> LineProtection {
        let mut reason_ids = Vec::new();
        for rule in rules() {
            if rule.regex.is_match(content) {
                reason_ids.push(rule.id);
            }
        }

        let lowered = if self.user_patterns.is_empty() {
            None
        } else {
            Some(content.to_lowercase())
        };
        let user_pattern = lowered.and_then(|line| {
            self.user_patterns
                .iter()
                .find(|(_, pattern)| line.contains(pattern))
                .map(|(original, _)| original.clone())
        });

        if is_command_context(content) {
            reason_ids.push("command-context");
        }

        LineProtection {
            protected: !reason_ids.is_empty() || user_pattern.is_some(),
            reason_ids,
            user_pattern,
        }
    }
}

fn is_command_context(content: &str) -> bool {
    static COMMAND: OnceLock<Regex> = OnceLock::new();
    let regex = COMMAND.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*(?:[$>]\s+\S|PS\s+[^>\r\n]*>\s*\S|(?:command|cmd|args|argv)\s*[:=]\s*\S|(?:cargo|npm|pnpm|yarn|pytest|jest|tsc|mvn|mvnw|gradle|gradlew|make|dotnet|python|python3|node|bun|deno|vite|webpack)\b)",
        )
        .expect("command-context regex must compile")
    });
    regex.is_match(content)
}

pub(crate) fn rule_label(id: &str) -> &'static str {
    static COMMAND: &str = "command context";
    static USER: &str = "user preserve pattern";
    if id == "command-context" {
        return COMMAND;
    }
    if id == "user-pattern" {
        return USER;
    }
    rules()
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.label)
        .unwrap_or("preservation rule")
}

/// Public metadata for the built-in preservation rules.
///
/// Regexes are intentionally not exposed here: the UI only needs stable labels
/// and identifiers. User patterns are literal, case-insensitive substrings.
#[derive(Debug, Clone, Serialize)]
pub struct ProtectionRuleInfo {
    pub id: &'static str,
    pub label: &'static str,
}

/// List built-in protection rules in display order, including command context.
pub fn protection_rule_catalog() -> Vec<ProtectionRuleInfo> {
    let mut catalog = rules()
        .iter()
        .map(|rule| ProtectionRuleInfo {
            id: rule.id,
            label: rule.label,
        })
        .collect::<Vec<_>>();
    catalog.push(ProtectionRuleInfo {
        id: "command-context",
        label: "command invocation or shell prompt",
    });
    catalog
}

/// Result of checking which lines a rule set protects.
#[derive(Debug, Clone, Serialize)]
pub struct ProtectionInspection {
    pub total_lines: usize,
    pub protected_lines: usize,
    pub reason_counts: BTreeMap<String, usize>,
    pub matches: Vec<ProtectedLine>,
    pub truncated: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProtectedLine {
    pub line_number: usize,
    pub text: String,
    pub reasons: Vec<String>,
}

/// Inspect local protection rules against text without compressing it.
///
/// `max_matches` bounds the response so a large pasted document cannot create an
/// unbounded API response. Counts still cover the whole input.
pub fn inspect_protection(
    text: &str,
    preserve_patterns: &[String],
    max_matches: usize,
) -> ProtectionInspection {
    let protector = Protector::new(preserve_patterns);
    let lines = split_content_lines(text);
    let mut protected_lines = 0usize;
    let mut reason_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut matches = Vec::new();

    for (index, content) in lines.iter().enumerate() {
        let info = protector.inspect_line(content, index == 0);
        if !info.protected {
            continue;
        }
        protected_lines += 1;
        let mut reasons: Vec<String> = info
            .reason_ids
            .iter()
            .map(|id| rule_label(id).to_string())
            .collect();
        if let Some(pattern) = &info.user_pattern {
            reasons.push(format!("user preserve pattern: {pattern}"));
        }
        for reason in &reasons {
            *reason_counts.entry(reason.clone()).or_insert(0) += 1;
        }
        if matches.len() < max_matches {
            matches.push(ProtectedLine {
                line_number: index + 1,
                text: truncate_for_display(content, 500),
                reasons,
            });
        }
    }

    ProtectionInspection {
        total_lines: lines.len(),
        protected_lines,
        reason_counts,
        truncated: protected_lines > matches.len(),
        matches,
        note: "Protection is heuristic and rule-based. It preserves matching evidence but does not prove that all important context is retained.".into(),
    }
}

fn truncate_for_display(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut result: String = text.chars().take(max_chars).collect();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protector() -> Protector {
        Protector::new(&[])
    }

    #[test]
    fn errors_and_file_locations_are_protected() {
        let p = protector();
        assert!(
            p.inspect_line("error[E0308]: mismatched types", false)
                .protected
        );
        assert!(p.inspect_line("  --> src/main.rs:10:5", false).protected);
        assert!(
            p.inspect_line("thread 'main' panicked at src/lib.rs:4:1", false)
                .protected
        );
        assert!(
            p.inspect_line(r#"File "app.py", line 22, in run"#, false)
                .protected
        );
        assert!(p.inspect_line("exit code: 101", false).protected);
        assert!(
            p.inspect_line("SECURITY: CVE-2026-1234 vulnerability found", false)
                .protected
        );
    }

    #[test]
    fn benign_progress_lines_are_not_protected() {
        let p = protector();
        assert!(
            !p.inspect_line("Downloading dependencies... 42%", false)
                .protected
        );
        assert!(!p.inspect_line("test_that_passes ... ok", false).protected);
    }

    #[test]
    fn user_patterns_are_literal_and_case_insensitive() {
        let p = Protector::new(&["AcmeCorp".into(), "a.b".into()]);
        assert!(p.inspect_line("tenant=acmecorp", false).protected);
        assert!(p.inspect_line("path=a.b", false).protected);
        // A dot is literal, not a regex wildcard.
        assert!(!p.inspect_line("axb", false).protected);
    }

    #[test]
    fn command_lines_are_preserved_as_context() {
        let p = protector();
        assert!(p.inspect_line("$ cargo test --workspace", false).protected);
        assert!(p.inspect_line("cargo test --workspace", false).protected);
        assert!(!p.inspect_line("just some text", false).protected);
    }

    #[test]
    fn inspection_counts_and_truncates() {
        let text = "error: one\nprogress 10%\nerror: two\n";
        let result = inspect_protection(text, &[], 1);
        assert_eq!(result.total_lines, 3);
        assert_eq!(result.protected_lines, 2);
        assert_eq!(result.matches.len(), 1);
        assert!(result.truncated);
    }
}
