//! Line-oriented, deterministic compression for logs and terminal output.

use std::collections::HashSet;
use std::sync::OnceLock;

use regex::{Regex, RegexSet};

use crate::protect::Protector;
use crate::types::{CompressionConfig, InputKind, Omissions, Strategy};

#[derive(Debug, Clone)]
struct Line {
    content: String,
    ending: String,
    protected: bool,
    context_sensitive: bool,
    generated: bool,
}

pub(crate) struct TextAttempt {
    pub output: String,
    pub omissions: Omissions,
    pub warnings: Vec<String>,
}

pub(crate) fn compress_text(
    input: &str,
    config: &CompressionConfig,
    kind: InputKind,
) -> TextAttempt {
    let mut lines = parse_lines(input);
    let dominant = dominant_line_ending(&lines);
    let protector = Protector::new(&config.preserve_patterns);

    let direct: Vec<bool> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| protector.inspect_line(&line.content, index == 0).protected)
        .collect();

    for (index, line) in lines.iter_mut().enumerate() {
        line.protected = direct[index];
        let previous = index > 0 && direct[index - 1];
        let next = index + 1 < direct.len() && direct[index + 1];
        line.context_sensitive = previous || next;
    }

    let mut omissions = Omissions {
        total_input_lines: lines.len(),
        ..Omissions::default()
    };
    let mut warnings = Vec::new();

    if config.collapse_passing_tests
        && matches!(config.strategy, Strategy::Balanced | Strategy::Aggressive)
    {
        let (next_lines, omitted, markers) = omit_passing_test_groups(lines, &dominant);
        lines = next_lines;
        if omitted > 0 {
            omissions.passing_test_lines_omitted = omitted;
            omissions.markers_inserted += markers;
            warnings.push(format!(
                "{omitted} passing test line(s) were omitted. Failing tests and summaries are preserved by the protection rules; use conservative mode to retain every test name."
            ));
        }
    }

    if config.collapse_progress_updates
        && matches!(config.strategy, Strategy::Balanced | Strategy::Aggressive)
    {
        let (next_lines, omitted, markers) = omit_progress_groups(lines, &dominant);
        lines = next_lines;
        if omitted > 0 {
            omissions.progress_lines_omitted = omitted;
            omissions.markers_inserted += markers;
            warnings.push(format!(
                "{omitted} progress update(s) were omitted; the final state of each progress group was retained."
            ));
        }
    }

    if config.collapse_repetitions {
        let threshold = match config.strategy {
            Strategy::Conservative => 3,
            Strategy::Balanced | Strategy::Aggressive => 2,
        };
        let (next_lines, runs, omitted, markers) =
            collapse_repeated_runs(lines, threshold, &dominant);
        lines = next_lines;
        if omitted > 0 {
            omissions.repeated_line_runs += runs;
            omissions.repeated_lines_omitted += omitted;
            omissions.markers_inserted += markers;
            warnings.push(format!(
                "{omitted} repeated line(s) were omitted while retaining one representative per run."
            ));
        }
    }

    if config.collapse_repetitions && config.strategy == Strategy::Aggressive {
        if kind == InputKind::StructuredLog {
            let (next_lines, omitted, markers) = collapse_normalized_log_runs(lines, &dominant);
            lines = next_lines;
            if omitted > 0 {
                omissions.normalized_log_lines_omitted += omitted;
                omissions.markers_inserted += markers;
                warnings.push(format!(
                    "{omitted} similar structured-log line(s) with varying timestamps, identifiers, or numbers were grouped. Only one representative was retained; review the diff."
                ));
            }
        }

        let (next_lines, omitted, markers) = remove_global_duplicate_lines(lines, &dominant);
        lines = next_lines;
        if omitted > 0 {
            omissions.repeated_lines_omitted += omitted;
            omissions.markers_inserted += markers;
            warnings.push(format!(
                "{omitted} non-consecutive duplicate line(s) were omitted globally. Use balanced or conservative mode when every occurrence carries meaning."
            ));
        }
    }

    omissions.total_output_lines = lines.len();
    let mut output = render_lines(&lines);
    preserve_trailing_newline(input, &mut output);

    TextAttempt {
        output,
        omissions,
        warnings,
    }
}

fn parse_lines(input: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for piece in input.split_inclusive('\n') {
        let (content, ending) = if let Some(value) = piece.strip_suffix("\r\n") {
            (value, "\r\n")
        } else if let Some(value) = piece.strip_suffix('\n') {
            (value, "\n")
        } else {
            (piece, "")
        };
        lines.push(Line {
            content: content.to_string(),
            ending: ending.to_string(),
            protected: false,
            context_sensitive: false,
            generated: false,
        });
    }
    lines
}

pub(crate) fn split_content_lines(text: &str) -> Vec<String> {
    parse_lines(text)
        .into_iter()
        .map(|line| line.content)
        .collect()
}

fn dominant_line_ending(lines: &[Line]) -> String {
    let crlf = lines.iter().filter(|line| line.ending == "\r\n").count();
    let lf = lines.iter().filter(|line| line.ending == "\n").count();
    if crlf > lf {
        "\r\n".into()
    } else {
        "\n".into()
    }
}

fn render_lines(lines: &[Line]) -> String {
    let mut output = String::new();
    for line in lines {
        output.push_str(&line.content);
        output.push_str(&line.ending);
    }
    output
}

fn preserve_trailing_newline(input: &str, output: &mut String) {
    if !input.ends_with('\n') {
        if output.ends_with("\r\n") {
            output.truncate(output.len() - 2);
        } else if output.ends_with('\n') {
            output.pop();
        }
    }
}

fn marker_line(content: String, ending: &str) -> Line {
    Line {
        content,
        ending: ending.to_string(),
        protected: true,
        context_sensitive: true,
        generated: true,
    }
}

fn can_omit(line: &Line) -> bool {
    !line.protected && !line.context_sensitive && !line.generated
}

fn omit_passing_test_groups(lines: Vec<Line>, dominant: &str) -> (Vec<Line>, usize, usize) {
    let mut output = Vec::with_capacity(lines.len());
    let mut omitted = 0usize;
    let mut markers = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        if is_passing_test_line(&lines[index]) && can_omit(&lines[index]) {
            let start = index;
            while index < lines.len()
                && is_passing_test_line(&lines[index])
                && can_omit(&lines[index])
            {
                index += 1;
            }
            let count = index - start;
            if count >= 2 {
                omitted += count;
                markers += 1;
                output.push(marker_line(
                    format!(
                        "[{count} passing test lines omitted; failing tests and summaries retained by protection rules]"
                    ),
                    dominant,
                ));
                continue;
            }
            output.extend(lines[start..index].iter().cloned());
            continue;
        }
        output.push(lines[index].clone());
        index += 1;
    }

    (output, omitted, markers)
}

fn is_passing_test_line(line: &Line) -> bool {
    static PASS_RULES: OnceLock<RegexSet> = OnceLock::new();
    static BLOCKERS: OnceLock<Regex> = OnceLock::new();
    let rules = PASS_RULES.get_or_init(|| {
        RegexSet::new([
            r"(?i)^\s*test\s+.+\s+\.\.\.\s+ok(?:\s+.*)?$",
            r"^\s*---\s+PASS:\s+\S+.*$",
            r"(?i)^\s*PASSED\s+\S+.*$",
            r"(?i)^\s*PASS\s+\S+.*$",
            r"^\s*\S+::\S+\s+PASSED(?:\s+.*)?$",
            r"^\s*[✓✔]\s+\S+.*$",
            r"(?i)^\s*ok\s+\d+\s+.*$",
            r"(?i)^\s*ok\s+\S+\s+\d+(?:\.\d+)?s(?:\s+coverage:.*)?$",
            r"(?i)^\s*\[\s*PASS\s*\]\s+\S+.*$",
        ])
        .expect("passing-test regex set must compile")
    });
    let blockers = BLOCKERS.get_or_init(|| {
        Regex::new(r"(?i)\b(?:fail(?:ed|ing|ure)?|error|panic|warn(?:ing)?|skip(?:ped)?|ignored)\b")
            .unwrap()
    });
    rules.is_match(&line.content) && !blockers.is_match(&line.content)
}

fn omit_progress_groups(lines: Vec<Line>, dominant: &str) -> (Vec<Line>, usize, usize) {
    static BAR: OnceLock<Regex> = OnceLock::new();
    static PERCENT: OnceLock<Regex> = OnceLock::new();
    static SPINNER: OnceLock<Regex> = OnceLock::new();

    let bar = BAR.get_or_init(|| Regex::new(r"\[\s*[#=>.\-]{8,}\s*\]").unwrap());
    let percent = PERCENT.get_or_init(|| {
        Regex::new(
            r"(?i)(?:\b(?:progress|downloading|installing|building|compiling|resolving|fetching|collecting|packages?|dependencies)\b.{0,100}\b\d{1,3}(?:\.\d+)?%|\b\d{1,3}(?:\.\d+)?%.{0,100}\b(?:progress|downloading|installing|building|compiling|resolving|fetching|collecting|packages?|dependencies)\b)",
        )
        .unwrap()
    });
    let spinner = SPINNER.get_or_init(|| Regex::new(r"^\s*[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]\s+\S+").unwrap());

    let is_progress = |line: &Line| {
        can_omit(line)
            && (bar.is_match(&line.content)
                || percent.is_match(&line.content)
                || spinner.is_match(&line.content))
    };

    let mut output = Vec::with_capacity(lines.len());
    let mut omitted = 0usize;
    let mut markers = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        if is_progress(&lines[index]) {
            let start = index;
            while index < lines.len() && is_progress(&lines[index]) {
                index += 1;
            }
            let count = index - start;
            if count >= 3 {
                let omitted_here = count - 1;
                omitted += omitted_here;
                markers += 1;
                output.push(marker_line(
                    format!("[{omitted_here} progress updates omitted; final state retained]"),
                    dominant,
                ));
                output.push(lines[index - 1].clone());
                continue;
            }
            output.extend(lines[start..index].iter().cloned());
            continue;
        }
        output.push(lines[index].clone());
        index += 1;
    }

    (output, omitted, markers)
}

fn collapse_repeated_runs(
    lines: Vec<Line>,
    threshold: usize,
    dominant: &str,
) -> (Vec<Line>, usize, usize, usize) {
    let mut output = Vec::with_capacity(lines.len());
    let mut runs = 0usize;
    let mut omitted = 0usize;
    let mut markers = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        if !can_omit(&lines[index]) {
            output.push(lines[index].clone());
            index += 1;
            continue;
        }
        let start = index;
        let content = &lines[index].content;
        index += 1;
        while index < lines.len() && can_omit(&lines[index]) && lines[index].content == *content {
            index += 1;
        }
        let count = index - start;
        if count >= threshold {
            runs += 1;
            omitted += count - 1;
            markers += 1;
            output.push(lines[start].clone());
            output.push(marker_line(
                format!(
                    "[{} repeated lines omitted; representative line retained]",
                    count - 1
                ),
                dominant,
            ));
        } else {
            output.extend(lines[start..index].iter().cloned());
        }
    }

    (output, runs, omitted, markers)
}

fn collapse_normalized_log_runs(lines: Vec<Line>, dominant: &str) -> (Vec<Line>, usize, usize) {
    let mut output = Vec::with_capacity(lines.len());
    let mut omitted = 0usize;
    let mut markers = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        if !can_omit(&lines[index]) || lines[index].content.trim().is_empty() {
            output.push(lines[index].clone());
            index += 1;
            continue;
        }
        let signature = normalized_log_signature(&lines[index].content);
        let start = index;
        index += 1;
        while index < lines.len()
            && can_omit(&lines[index])
            && normalized_log_signature(&lines[index].content) == signature
        {
            index += 1;
        }
        let count = index - start;
        if count >= 3 {
            omitted += count - 1;
            markers += 1;
            output.push(lines[start].clone());
            output.push(marker_line(
                format!(
                    "[{} similar structured-log lines omitted; representative line retained]",
                    count - 1
                ),
                dominant,
            ));
        } else {
            output.extend(lines[start..index].iter().cloned());
        }
    }

    (output, omitted, markers)
}

fn normalized_log_signature(content: &str) -> String {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            (
                Regex::new(
                    r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
                )
                .unwrap(),
                "<uuid>",
            ),
            (
                Regex::new(
                    r"\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?\b",
                )
                .unwrap(),
                "<timestamp>",
            ),
            (
                Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(),
                "<ip>",
            ),
            (
                Regex::new(r"\b(?:0x)?[0-9a-fA-F]{8,}\b").unwrap(),
                "<hex>",
            ),
            (
                Regex::new(r"\b\d+(?:\.\d+)?\b").unwrap(),
                "<n>",
            ),
        ]
    });

    let mut result = content.to_string();
    for (regex, replacement) in patterns {
        result = regex.replace_all(&result, *replacement).into_owned();
    }
    result
}

fn remove_global_duplicate_lines(lines: Vec<Line>, dominant: &str) -> (Vec<Line>, usize, usize) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut output: Vec<Line> = Vec::with_capacity(lines.len());
    let mut omitted = 0usize;
    let mut marker_index: Option<usize> = None;

    for line in lines {
        if can_omit(&line) && !seen.insert(line.content.clone()) {
            omitted += 1;
            if marker_index.is_none() {
                marker_index = Some(output.len());
            }
            continue;
        }
        output.push(line);
    }

    let mut markers = 0usize;
    if omitted > 0 {
        let index = marker_index.unwrap_or(0).min(output.len());
        output.insert(
            index,
            marker_line(
                format!("[{omitted} duplicate lines omitted; first occurrence of each retained]"),
                dominant,
            ),
        );
        markers = 1;
    }

    (output, omitted, markers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InputFormat;

    fn config(strategy: Strategy) -> CompressionConfig {
        CompressionConfig {
            strategy,
            input_format: InputFormat::Auto,
            ..CompressionConfig::default()
        }
    }

    fn run(text: &str, strategy: Strategy) -> TextAttempt {
        compress_text(text, &config(strategy), InputKind::PlainText)
    }

    #[test]
    fn conservative_collapses_only_consecutive_runs() {
        let input = "same\nsame\nsame\nother\nsame\n";
        let attempt = run(input, Strategy::Conservative);
        assert!(attempt.output.contains("same"));
        assert!(attempt.output.contains("2 repeated lines omitted"));
        assert_eq!(attempt.omissions.repeated_lines_omitted, 2);
        upcount_assert(&attempt);
    }

    fn upcount_assert(attempt: &TextAttempt) {
        let remaining = attempt.output.matches("same").count();
        assert_eq!(remaining, 2);
    }

    #[test]
    fn balanced_does_not_deduplicate_non_consecutive_lines() {
        let input = "same\nother\nsame\n";
        let attempt = run(input, Strategy::Balanced);
        assert_eq!(attempt.omissions.repeated_lines_omitted, 0);
        assert_eq!(attempt.output, input);
    }

    #[test]
    fn aggressive_removes_non_consecutive_duplicates() {
        let input = "same\nother\nsame\n";
        let attempt = run(input, Strategy::Aggressive);
        assert_eq!(attempt.omissions.repeated_lines_omitted, 1);
        assert!(attempt.output.contains("1 duplicate lines omitted"));
        assert_eq!(attempt.output.matches("same").count(), 1);
    }

    #[test]
    fn protected_lines_are_never_collapsed() {
        let input = "error: same failure\nerror: same failure\nerror: same failure\n";
        let attempt = run(input, Strategy::Aggressive);
        assert_eq!(attempt.omissions.total_omitted(), 0);
        assert_eq!(attempt.output, input);
    }

    #[test]
    fn repeated_run_boundary_context_is_retained_while_noise_is_collapsed() {
        let input = "wait\nwait\nwait\nwait\nerror: stop\n";
        let attempt = run(input, Strategy::Balanced);
        assert_eq!(attempt.omissions.repeated_lines_omitted, 2);
        assert_eq!(attempt.output.matches("wait").count(), 2);
        assert!(attempt.output.contains("error: stop"));
        assert!(attempt.output.contains("2 repeated lines omitted"));
    }

    #[test]
    fn adjacent_diagnostic_context_is_not_removed() {
        let input = "progress 10%\nprogress 20%\nerror: boom\nprogress 30%\nprogress 40%\n";
        let attempt = run(input, Strategy::Balanced);
        // No group of three safe progress lines exists because both groups touch
        // the protected error line.
        assert_eq!(attempt.omissions.progress_lines_omitted, 0);
    }

    #[test]
    fn passing_test_groups_are_collapsed_but_failure_lines_remain() {
        let input = "\
test parser::a ... ok
test parser::b ... ok
test parser::c ... ok
test parser::d ... ok
test parser::e ... FAILED
test result: FAILED. 4 passed; 1 failed
";
        let attempt = run(input, Strategy::Balanced);
        assert!(attempt.output.contains("3 passing test lines omitted"));
        assert!(attempt.output.contains("FAILED"));
        assert!(attempt.output.contains("test result"));
        assert_eq!(attempt.omissions.passing_test_lines_omitted, 3);
    }

    #[test]
    fn crlf_is_preserved() {
        let input = "same\r\nsame\r\nsame\r\n";
        let attempt = run(input, Strategy::Conservative);
        assert!(attempt.output.contains("\r\n"));
        assert!(!attempt.output.contains("\n\n"));
        assert!(attempt.output.ends_with("\r\n"));
    }

    #[test]
    fn unicode_content_is_handled() {
        let input = "警告: 同じ問題\n進捗 10%\n進捗 20%\n進捗 30%\n完了\n";
        let attempt = run(input, Strategy::Balanced);
        assert!(attempt.output.contains("警告"));
        assert!(attempt.output.contains("完了"));
    }

    #[test]
    fn no_trailing_newline_is_preserved() {
        let input = "same\nsame\nsame";
        let attempt = run(input, Strategy::Conservative);
        assert!(!attempt.output.ends_with('\n'));
    }

    #[test]
    fn empty_input_is_handled() {
        let attempt = run("", Strategy::Aggressive);
        assert_eq!(attempt.output, "");
        assert_eq!(attempt.omissions.total_input_lines, 0);
    }
}
