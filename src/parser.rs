//! Minimal log parser — only detects prompt expected total from print_timing lines.

#![allow(dead_code)]

use std::sync::LazyLock;

use regex::Regex;

/// Matches: "prompt processing, n_tokens = 44032, progress = 0.91, t = 73.95 s / 595.46 tokens per second"
static PRINT_TIMING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"prompt processing,\s*n_tokens\s*=\s*(\d+),\s*progress\s*=\s*([\d.]+)").unwrap()
});

/// If this log line contains prompt processing progress, return the expected total token count.
/// expected_total = n_tokens / progress
pub fn parse_prompt_expected_total(line: &str) -> Option<u64> {
    let caps = PRINT_TIMING_RE.captures(line)?;
    let n_tokens: u64 = caps.get(1)?.as_str().parse().ok()?;
    let progress: f64 = caps.get(2)?.as_str().parse().ok()?;

    if progress > 0.0 && progress < 1.0 {
        Some((n_tokens as f64 / progress) as u64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_print_timing() {
        let line = "slot print_timing: id  1 | task 162 | prompt processing, n_tokens =  44032, progress = 0.91, t =  73.95 s / 595.46 tokens per second";
        let total = parse_prompt_expected_total(line).expect("should parse");
        assert_eq!(total, 48386); // 44032 / 0.91 = 48386.81
    }

    #[test]
    fn test_ignores_non_matching_lines() {
        assert!(parse_prompt_expected_total("some random log line").is_none());
    }

    #[test]
    fn test_ignores_progress_at_1() {
        let line = "prompt processing, n_tokens = 1000, progress = 1.0";
        assert!(parse_prompt_expected_total(line).is_none());
    }
}
