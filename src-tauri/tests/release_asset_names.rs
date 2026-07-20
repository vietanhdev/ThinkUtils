//! The download page must look for the filenames the release actually produces.
//!
//! Two files have to agree and neither imports the other: `release.yml` renames
//! each artifact after building it, and `docs/download.md` matches release
//! assets by suffix to build its download buttons.
//!
//! When they disagree nothing fails. `url()` falls back to the releases page, so
//! every button still works — it just stops linking the actual package, silently,
//! for whichever architecture drifted. The page looks entirely correct.
//!
//! Tauri makes this easy to get wrong by naming architectures differently per
//! format, and inconsistently between them:
//!
//! ```text
//!   format      x86_64    aarch64
//!   .deb        amd64     arm64
//!   .rpm        x86_64    aarch64
//!   .AppImage   amd64     aarch64     <- deb's spelling on x86_64, rpm's on ARM
//! ```
//!
//! That last row already cost a red CI run: the launch suite derived AppImage
//! from the deb architecture, which is correct on x86_64 by coincidence and
//! wrong on aarch64.

use std::path::PathBuf;

fn repo_file(rel: &str) -> String {
    let p = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {}", p.display(), e))
}

/// `(arch, deb_arch, rpm_arch)` as declared by release.yml's build matrix.
fn matrix_arches() -> Vec<(String, String, String)> {
    let release = repo_file(".github/workflows/release.yml");
    let mut out = Vec::new();

    for line in release.lines() {
        let l = line.trim();
        if !l.starts_with("- { arch:") {
            continue;
        }
        let field = |name: &str| -> Option<String> {
            let at = l.find(&format!("{}:", name))? + name.len() + 1;
            Some(
                l[at..]
                    .trim_start()
                    .split([',', '}'])
                    .next()?
                    .trim()
                    .trim_matches('"')
                    .to_string(),
            )
        };
        if let (Some(a), Some(d), Some(r)) = (field("arch"), field("deb_arch"), field("rpm_arch")) {
            out.push((a, d, r));
        }
    }

    assert_eq!(
        out.len(),
        2,
        "expected two architectures in release.yml's build matrix, parsed {:?}",
        out
    );
    out
}

/// The exact asset names a release publishes, per architecture.
fn published_names(deb: &str, rpm: &str) -> [String; 3] {
    let release = repo_file(".github/workflows/release.yml");
    assert_predicates_unchanged();

    // Assert the rename templates are still the ones this test models, rather
    // than silently checking names nothing produces.
    for template in [
        "thinkutils_${V}_${{ matrix.deb_arch }}.deb",
        "thinkutils-${V}.${{ matrix.rpm_arch }}.rpm",
        "thinkutils_${V}_${{ matrix.deb_arch }}.AppImage",
    ] {
        assert!(
            release.contains(template),
            "release.yml no longer renames to {} - this test is modelling names that are not produced",
            template
        );
    }

    [
        format!("thinkutils_9.9.9_{}.deb", deb),
        format!("thinkutils-9.9.9.{}.rpm", rpm),
        format!("thinkutils_9.9.9_{}.AppImage", deb),
    ]
}

/// The arch table `docs/download.md` actually declares, parsed from the page.
///
/// Parsed rather than reconstructed from the matrix: building the expected
/// suffixes out of the matrix values and then comparing them to matrix-derived
/// filenames compares a thing to itself and passes no matter what the page says.
fn page_arches() -> Vec<(String, String, String)> {
    let page = repo_file("docs/download.md");
    let mut out = Vec::new();

    for line in page.lines() {
        let l = line.trim();
        // e.g.  aarch64: { label: "ARM (aarch64)", deb: "arm64", rpm: "aarch64" },
        if !l.contains("deb:") || !l.contains("rpm:") || !l.contains("label:") {
            continue;
        }
        let Some((arch, rest)) = l.split_once(':') else {
            continue;
        };
        let field = |name: &str| -> Option<String> {
            let at = rest.find(&format!("{}:", name))? + name.len() + 1;
            Some(
                rest[at..]
                    .trim_start()
                    .trim_start_matches('"')
                    .split('"')
                    .next()?
                    .to_string(),
            )
        };
        if let (Some(d), Some(r)) = (field("deb"), field("rpm")) {
            out.push((arch.trim().to_string(), d, r));
        }
    }

    assert_eq!(
        out.len(),
        2,
        "expected two architectures in download.md's ARCHES table, parsed {:?}",
        out
    );
    out
}

#[test]
fn the_download_page_matches_every_published_asset() {
    let matrix = matrix_arches();
    let page = page_arches();

    for (arch, deb, rpm) in &matrix {
        let (_, page_deb, page_rpm) = page
            .iter()
            .find(|(a, _, _)| a == arch)
            .unwrap_or_else(|| panic!("download.md has no entry for {}", arch));

        // Names the release publishes, against suffixes the PAGE declares.
        let names = published_names(deb, rpm);
        let suffixes = [
            format!("_{}.deb", page_deb),
            format!(".{}.rpm", page_rpm),
            format!("_{}.AppImage", page_deb),
        ];

        for (name, suffix) in names.iter().zip(suffixes.iter()) {
            assert!(
                name.ends_with(suffix.as_str()),
                "on {}: release publishes {} but the download page looks for *{} - \
                 the button would silently fall back to the releases page",
                arch,
                name,
                suffix
            );
        }
    }
}

/// The page's architecture table must cover exactly the matrix, or a built
/// architecture has no way to be selected and its packages are unreachable.
#[test]
fn the_download_page_offers_every_built_architecture() {
    let page = repo_file("docs/download.md");

    for (arch, deb, rpm) in matrix_arches() {
        assert!(
            page.contains(&format!("{}:", arch)),
            "download.md has no entry for {}, which release.yml builds",
            arch
        );
        assert!(
            page.contains(&format!("deb: \"{}\"", deb))
                && page.contains(&format!("rpm: \"{}\"", rpm)),
            "download.md's {} entry does not carry deb: {} / rpm: {}",
            arch,
            deb,
            rpm
        );
    }
}

/// The page must still match by suffix at all. If the predicates are rewritten,
/// the parsing above models something the page no longer does.
fn assert_predicates_unchanged() {
    let page = repo_file("docs/download.md");
    for pred in [
        "n.endsWith(`_${suffix.value.deb}.deb`)",
        "n.endsWith(`.${suffix.value.rpm}.rpm`)",
        "n.endsWith(`_${suffix.value.deb}.AppImage`)",
    ] {
        assert!(
            page.contains(pred),
            "download.md no longer uses the predicate {} - this test models matching it does not do",
            pred
        );
    }
}
