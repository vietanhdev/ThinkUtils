//! `cancel-in-progress` must be a literal, never a comparison expression.
//!
//! This looks obviously correct and does not work:
//!
//! ```yaml
//! cancel-in-progress: ${{ github.ref != 'refs/heads/main' }}
//! ```
//!
//! The expression renders to the *string* `"false"`, and a non-empty string is
//! truthy in that position, so the branch it was meant to protect gets cancelled
//! anyway. It failed silently for exactly as long as nobody merged two things in
//! quick succession, then quietly killed four runs on `main` — each with zero
//! jobs recorded, leaving those commits with no evidence they ever built.
//!
//! Conditional behaviour belongs in the concurrency GROUP, where a per-SHA
//! component simply leaves nothing to supersede.
//!
//! Lives outside `.github/` so it cannot match its own explanation.

use std::path::PathBuf;

fn workflows_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.github/workflows"))
}

fn workflow_files() -> Vec<(String, String)> {
    let dir = workflows_dir();
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e));

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("yml") {
            continue;
        }
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let content = std::fs::read_to_string(&p).expect("workflow is readable");
        out.push((name, content));
    }
    assert!(!out.is_empty(), "found no workflows to scan");
    out
}

/// Directive lines only. The `#` comments in ci.yml document the broken form on
/// purpose, and a naive scan flags the warning as loudly as the mistake.
fn directives(content: &str) -> impl Iterator<Item = (usize, &str)> {
    content
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with('#'))
}

#[test]
fn cancel_in_progress_is_never_an_expression() {
    let mut violations = Vec::new();

    for (name, content) in workflow_files() {
        for (i, line) in directives(&content) {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            if key.trim() != "cancel-in-progress" {
                continue;
            }
            let value = value.trim();
            if value != "true" && value != "false" {
                violations.push(format!("{}:{}: {}", name, i + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "cancel-in-progress must be a literal true/false. An expression renders \
         to a string, and any non-empty string is truthy -- so \
         `${{{{ github.ref != 'refs/heads/main' }}}}` cancels main rather than \
         protecting it. Put the condition in the concurrency group instead:\n  {}",
        violations.join("\n  ")
    );
}

/// The replacement only works if the group actually varies per commit on main.
/// A literal `cancel-in-progress: true` with a per-ref group would cancel main
/// on every push — strictly worse than what this replaced.
#[test]
fn main_runs_cannot_be_superseded() {
    let ci = workflow_files()
        .into_iter()
        .find(|(n, _)| n == "ci.yml")
        .expect("ci.yml exists")
        .1;

    let group = directives(&ci)
        .map(|(_, l)| l)
        .find(|l| l.trim_start().starts_with("group:"))
        .expect("ci.yml declares a concurrency group");

    assert!(
        group.contains("github.sha"),
        "the concurrency group must include github.sha for main, or a second \
         push cancels the first run: {}",
        group.trim()
    );
    assert!(
        group.contains("refs/heads/main"),
        "the per-SHA component must be conditional on main, otherwise every \
         branch push gets its own group and force-pushes stop superseding: {}",
        group.trim()
    );
}

/// A scanner that reads nothing passes for the wrong reason.
#[test]
fn the_scan_sees_real_workflow_content() {
    let files = workflow_files();
    assert!(
        files.len() >= 2,
        "expected several workflows, saw {}",
        files.len()
    );
    assert!(
        files.iter().any(|(_, c)| directives(c).count() > 20),
        "no workflow yielded a meaningful number of directive lines"
    );
    assert!(
        files.iter().any(|(n, _)| n == "ci.yml"),
        "ci.yml should be among the scanned workflows"
    );
}
