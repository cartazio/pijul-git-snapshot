use std::path::{Path, PathBuf};

/// Produce a PathBuf with the relative path from cwd to the given file
///
/// We assume cwd is a folder and file is a file. We also assume they
/// share a common root (at least the repository root).
///
/// cwd=/a/b/c/e
/// fil=/a/b/c/d/foo.rs
///
/// relativize(cwd, fil) #=> ../d/foo.rs
pub fn relativize(cwd: &Path, file: &Path) -> PathBuf {
    let mut p = PathBuf::new();
    let mut c1 = cwd.components();
    let mut c2 = file.parent().unwrap().components();
    loop {
        match (c1.next(), c2.next()) {
            (Some(r1), Some(r2)) if r1 != r2 => {
                // paths diverge
                p.push("..");
                for _ in c1 {
                    p.push("..");
                }
                p.push(r2.as_os_str());
                p.push(c2.as_path());
                break;
            }
            (None, Some(r2)) => {
                // Remaining stuff is relative path
                p.push(r2.as_os_str());
                p.push(c2.as_path());
                break;
            }
            (Some(_), None) => {
                p.push("..");
                for _ in c1 {
                    p.push("..");
                }
                break;
            }
            (None, None) => {
                break;
            }
            (Some(_), Some(_)) => {}
        }
    }
    let name = file.file_name().unwrap();
    p.push(name);
    p.set_file_name(name);
    debug!("rel({}, {})={}", cwd.display(), file.display(), p.display());
    p
}

#[cfg(test)]
mod test {
    use super::relativize;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_relativize() {
        let cases = vec![
            ("/a/b/c", "/a/b/c/foo.rs", "foo.rs"),
            ("/a/b/c", "/a/b/c/d/foo.rs", "d/foo.rs"),
            ("/a/b/c/e", "/a/b/c/foo.rs", "../foo.rs"),
            ("/a/b/c/e", "/a/b/c/d/foo.rs", "../d/foo.rs"),
            ("/a/b/c/d/e", "/a/b/c/foo.rs", "../../foo.rs"),
            (
                "/home/foo/rust/pijul",
                "/home/foo/rust/pijul/Cargo.lock",
                "Cargo.lock",
            ),
        ];
        for (root, file, expected) in cases {
            assert_eq!(
                PathBuf::from(expected),
                relativize(&Path::new(root), &Path::new(file))
            );
        }
    }
}
