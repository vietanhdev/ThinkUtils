//! Battery threshold attribute names must be spelled in exactly one place.
//!
//! The kernel's generic API calls them `charge_control_{start,end}_threshold`;
//! thinkpad_acpi's older interface calls them `charge_{start,stop}_threshold`.
//! Which pair a machine exposes depends on its kernel and model, so
//! `battery::threshold_paths()` probes for the pair that exists and every other
//! module is supposed to go through it.
//!
//! That rule was already written in a doc comment and already violated:
//! permissions.rs granted access to one pair while battery.rs wrote the other,
//! so "Grant Permissions" reported success and battery changes still fell
//! through to a password prompt on every use. mcp.rs named a third combination
//! and reported N/A for thresholds that worked.
//!
//! None of that fails a build or a test — it just silently does the wrong thing
//! on hardware the developer does not have. Hence this guard.
//!
//! Lives in its own integration-test file deliberately: a guard that greps
//! source for a literal, while itself containing that literal, matches itself.

use std::path::{Path, PathBuf};

/// The file allowed to name these attributes, because it owns the lookup table.
const OWNER: &str = "battery.rs";

fn src_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
}

/// Build the forbidden literals at runtime from fragments, so this file never
/// contains the strings it searches for.
fn attribute_literals() -> Vec<String> {
    let generic = format!("charge_control_{}_threshold", "start");
    let generic_end = format!("charge_control_{}_threshold", "end");
    let legacy = format!("charge_{}_threshold", "start");
    let legacy_stop = format!("charge_{}_threshold", "stop");
    vec![generic, generic_end, legacy, legacy_stop]
}

fn rust_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let dir = src_dir();
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e));
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    assert!(!out.is_empty(), "found no Rust sources to scan");
    out
}

/// Code before `#[cfg(test)]`, with `//` comments stripped.
///
/// Comments in these modules explain *why* the names are centralised and quote
/// them while doing so; a naive search matches the explanation as readily as a
/// violation.
fn production_code(content: &str) -> String {
    content
        .split("#[cfg(test)]")
        .next()
        .unwrap_or("")
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn only_battery_rs_spells_the_threshold_attribute_names() {
    let literals = attribute_literals();
    let mut violations = Vec::new();

    for path in rust_sources() {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == OWNER {
            continue;
        }
        let content = std::fs::read_to_string(&path).expect("source is readable");
        let code = production_code(&content);

        for (i, line) in code.lines().enumerate() {
            for lit in &literals {
                if line.contains(lit.as_str()) {
                    violations.push(format!("{}:{}: {}", name, i + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "these modules name battery threshold attributes directly instead of \
         calling battery::threshold_paths(), which is how they drifted apart \
         before:\n  {}",
        violations.join("\n  ")
    );
}

/// The guard above is only meaningful if the owner really does define both
/// spellings — otherwise it enforces routing to a lookup that cannot resolve.
#[test]
fn battery_rs_defines_both_naming_conventions() {
    let owner = std::fs::read_to_string(src_dir().join(OWNER)).expect("battery.rs is readable");
    let code = production_code(&owner);

    for lit in attribute_literals() {
        assert!(
            code.contains(lit.as_str()),
            "battery.rs should define the {} attribute so threshold_paths() can \
             probe for it",
            lit
        );
    }
}

/// A sanity check on the scanner itself: if it cannot see the owner's own
/// literals, its silence about other files proves nothing.
#[test]
fn the_scanner_can_actually_find_these_literals() {
    let owner_path = src_dir().join(OWNER);
    assert!(Path::new(&owner_path).exists(), "battery.rs must exist");

    let found = attribute_literals()
        .iter()
        .filter(|lit| {
            production_code(&std::fs::read_to_string(&owner_path).unwrap()).contains(lit.as_str())
        })
        .count();

    assert_eq!(
        found,
        attribute_literals().len(),
        "the scan found {} of {} literals in the one file guaranteed to contain \
         them, so a clean result elsewhere would be meaningless",
        found,
        attribute_literals().len()
    );
}
