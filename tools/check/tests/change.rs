use plectron_check::{Change, Kind, Source, parse};
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn statuses() {
    let changes = parse(
        "A\ta.rs\nM\tb.rs\nT\tc.rs\nD\td.rs\nR100\told.rs\tnew.rs\nC100\tsource.rs\tcopy.rs\n",
    )
    .unwrap();
    assert_eq!(changes[0].kind, Kind::Added);
    assert_eq!(changes[1].kind, Kind::Modified);
    assert_eq!(changes[2].kind, Kind::Modified);
    assert_eq!(changes[3].kind, Kind::Deleted);
    assert_eq!(changes[4].kind, Kind::Renamed);
    assert_eq!(changes[5].kind, Kind::Copied);
    assert_eq!(
        changes[4].previous.as_deref().unwrap().to_str(),
        Some("old.rs")
    );
    assert_eq!(changes[5].path.to_str(), Some("copy.rs"));
}

fn file(content: &[u8]) -> Result<Vec<Change>, String> {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("changes");
    std::fs::write(&input, content).unwrap();
    Source::File(input).read(directory.path())
}

#[test]
fn records() {
    let changes = file(
        b"A\0a\tb.rs\0M\0line\nbreak.rs\0T\0t.rs\0D\0d.rs\0R087\0old\t.rs\0new\n.rs\0C100\0s.rs\0c.rs\0",
    )
    .unwrap();
    let kinds: Vec<_> = changes.iter().map(|change| change.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::Added,
            Kind::Modified,
            Kind::Modified,
            Kind::Deleted,
            Kind::Renamed,
            Kind::Copied
        ]
    );
    assert_eq!(changes[0].path, Path::new("a\tb.rs"));
    assert_eq!(changes[1].path, Path::new("line\nbreak.rs"));
    assert_eq!(changes[4].previous.as_deref(), Some(Path::new("old\t.rs")));
    assert_eq!(changes[4].path, Path::new("new\n.rs"));
    assert_eq!(changes[5].previous.as_deref(), Some(Path::new("s.rs")));
    assert!(file(b"").unwrap().is_empty());
}

#[test]
fn malformed() {
    let unsupported = file(b"X\0a.rs\0").unwrap_err();
    assert!(
        unsupported.contains("unsupported change status"),
        "{unsupported}"
    );
    let empty = file(b"\0a.rs\0").unwrap_err();
    assert!(empty.contains("empty change status"), "{empty}");
    let missing = file(b"M\0").unwrap_err();
    assert!(missing.contains("incomplete change record"), "{missing}");
    let half = file(b"R100\0old.rs\0").unwrap_err();
    assert!(half.contains("incomplete change record"), "{half}");
    let unterminated = file(b"M\0a.rs").unwrap_err();
    assert!(
        unterminated.contains("missing NUL terminator"),
        "{unterminated}"
    );
    let text = file(b"M\ta.rs\tb.rs\n").unwrap_err();
    assert!(text.contains("invalid change record"), "{text}");
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=check",
            "-c",
            "user.email=check@invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

#[test]
fn special() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    git(root, &["init", "-q"]);
    let name = "tab\tand\nnewline.rs";
    std::fs::write(root.join(name), "one\n").unwrap();
    std::fs::write(root.join("plain.rs"), "plain\n").unwrap();
    git(root, &["add", "--all"]);

    let staged = Source::Staged.read(root).unwrap();
    let mut paths: Vec<_> = staged.iter().map(|change| change.path.clone()).collect();
    paths.sort();
    assert_eq!(paths, [PathBuf::from("plain.rs"), PathBuf::from(name)]);
    assert!(staged.iter().all(|change| change.kind == Kind::Added));

    git(root, &["commit", "-q", "-m", "base"]);
    let renamed = "renamed\twith\nbreaks.rs";
    git(root, &["mv", name, renamed]);
    let staged = Source::Staged.read(root).unwrap();
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].kind, Kind::Renamed);
    assert_eq!(staged[0].previous.as_deref(), Some(Path::new(name)));
    assert_eq!(staged[0].path, Path::new(renamed));

    std::fs::write(root.join("plain.rs"), "changed\n").unwrap();
    let worktree = Source::Worktree.read(root).unwrap();
    assert_eq!(worktree.len(), 1);
    assert_eq!(worktree[0].kind, Kind::Modified);
    assert_eq!(worktree[0].path, Path::new("plain.rs"));

    git(root, &["commit", "-q", "--all", "-m", "rename"]);
    let range = Source::Range("HEAD~1..HEAD".into()).read(root).unwrap();
    let moved = range
        .iter()
        .find(|change| change.kind == Kind::Renamed)
        .unwrap();
    assert_eq!(moved.path, Path::new(renamed));

    let failure = Source::Range("no-such-revision".into())
        .read(root)
        .unwrap_err();
    assert!(failure.contains("git diff"), "{failure}");
}

#[cfg(unix)]
#[test]
fn bytes() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    git(root, &["init", "-q"]);
    let name = OsStr::from_bytes(b"latin\xff1.rs");
    std::fs::write(root.join(name), "x\n").unwrap();
    git(root, &["add", "--all"]);
    let staged = Source::Staged.read(root).unwrap();
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].path.as_os_str(), name);
}
