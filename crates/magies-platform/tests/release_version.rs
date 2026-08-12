//! Covers deciding whether a published release is newer than this build.

use magies_platform::release::{ReleaseVersion, ReleaseVersionError, UpdateStatus};

#[test]
fn a_published_version_can_be_newer_older_or_the_same() {
    let current = ReleaseVersion::parse("0.1.0").unwrap();

    for (published, expected) in [
        ("0.2.0", UpdateStatus::UpdateAvailable),
        ("0.1.1", UpdateStatus::UpdateAvailable),
        ("1.0.0", UpdateStatus::UpdateAvailable),
        ("0.1.0", UpdateStatus::UpToDate),
        ("0.0.9", UpdateStatus::UpToDate),
    ] {
        let published = ReleaseVersion::parse(published).unwrap();

        assert_eq!(
            current.compare(&published),
            expected,
            "{current:?} against {published:?}"
        );
    }
}

#[test]
fn the_tag_prefix_release_tags_carry_is_accepted() {
    // Releases are tagged `v0.1.0`; comparing the tag against a bare version
    // would report every release as unparseable.
    assert_eq!(
        ReleaseVersion::parse("v1.2.3").unwrap(),
        ReleaseVersion::parse("1.2.3").unwrap()
    );
}

#[test]
fn a_component_that_is_not_a_number_is_a_typed_error() {
    for value in ["", "v", "1.2", "1.2.3.4", "1.2.x", "next"] {
        assert!(
            matches!(
                ReleaseVersion::parse(value),
                Err(ReleaseVersionError::Malformed { .. })
            ),
            "{value:?} must not parse as a version"
        );
    }
}

#[test]
fn a_prerelease_suffix_is_refused_rather_than_guessed() {
    // `0.2.0-rc.1` is older than `0.2.0` by semver, and treating it as equal
    // would offer a release candidate as a stable update.
    assert!(matches!(
        ReleaseVersion::parse("0.2.0-rc.1"),
        Err(ReleaseVersionError::Malformed { .. })
    ));
}

#[test]
fn ordering_compares_numbers_rather_than_text() {
    let current = ReleaseVersion::parse("0.9.0").unwrap();
    let published = ReleaseVersion::parse("0.10.0").unwrap();

    // "0.10.0" sorts before "0.9.0" as text, which would hide the update.
    assert_eq!(current.compare(&published), UpdateStatus::UpdateAvailable);
}
