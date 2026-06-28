use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// Global cache of resolved `.git` paths to branch names.
/// Key: the canonical `.git` directory path
/// Value: branch name, or None if detached head
static BRANCH_CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

/// Parse the branch name out of a `.git/HEAD` file body.
/// "ref: refs/heads/main\n" -> Some("main"); a raw sha (detached) -> None.
pub fn parse_head(head_contents: &str) -> Option<String> {
    let line = head_contents.trim();
    let rest = line.strip_prefix("ref:")?.trim();
    rest.strip_prefix("refs/heads/").map(|b| b.to_string())
}

/// Resolve the `.git` path, handling symlinks and worktrees.
/// If `.git` is a file with "gitdir: ..." content, follow it.
fn resolve_git_path(repo_path: &Path) -> std::io::Result<String> {
    let git_path = repo_path.join(".git");
    let metadata = fs::metadata(&git_path)?;

    if metadata.is_dir() {
        return Ok(git_path.canonicalize()?.to_string_lossy().into_owned());
    }

    // `.git` is a file; read it to get the gitdir path
    let contents = fs::read_to_string(&git_path)?;
    for line in contents.lines() {
        if let Some(gitdir) = line.strip_prefix("gitdir: ") {
            // gitdir can be absolute or relative to the `.git` file
            let gitdir_path = if Path::new(gitdir).is_absolute() {
                Path::new(gitdir).to_path_buf()
            } else {
                git_path.parent().unwrap_or(Path::new(".")).join(gitdir)
            };
            return Ok(gitdir_path.canonicalize()?.to_string_lossy().into_owned());
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "no gitdir found in .git file",
    ))
}

/// Internal: Get the current branch name for a repo using Path, with full error handling.
fn branch_for_path(repo_path: &Path) -> std::io::Result<Option<String>> {
    // Resolve the .git path
    let git_path = resolve_git_path(repo_path)?;

    // Check cache
    {
        let cache = BRANCH_CACHE.lock().unwrap();
        if let Some(ref map) = *cache {
            if let Some(branch) = map.get(&git_path) {
                return Ok(branch.clone());
            }
        }
    }

    // Cache miss: read .git/HEAD
    let head_path = format!("{}/HEAD", git_path);
    let head_contents = fs::read_to_string(head_path)?;
    let branch = parse_head(&head_contents);

    // Store in cache
    {
        let mut cache = BRANCH_CACHE.lock().unwrap();
        if cache.is_none() {
            *cache = Some(HashMap::new());
        }
        if let Some(ref mut map) = *cache {
            map.insert(git_path, branch.clone());
        }
    }

    Ok(branch)
}

/// Get the current branch name for a repo (best-effort, no error propagation).
/// Returns None on any error (missing .git, I/O failure, detached HEAD, etc.).
pub fn branch_for(cwd: &str) -> Option<String> {
    branch_for_path(Path::new(cwd)).ok().flatten()
}

/// Clear the branch name cache. Useful for testing or forcing a refresh.
pub fn invalidate() {
    if let Ok(mut cache) = BRANCH_CACHE.lock() {
        if let Some(ref mut map) = cache.as_mut() {
            map.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn parses_branch_ref() {
        assert_eq!(parse_head("ref: refs/heads/feat/notch-hud\n"), Some("feat/notch-hud".into()));
        assert_eq!(parse_head("ref: refs/heads/main"), Some("main".into()));
    }

    #[test]
    fn detached_head_is_none() {
        assert_eq!(parse_head("a1b2c3d4e5f6\n"), None);
    }

    #[test]
    fn branch_for_normal_git_dir() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let repo_path = temp.path();
        let git_dir = repo_path.join(".git");
        fs::create_dir(&git_dir)?;
        let mut head = fs::File::create(git_dir.join("HEAD"))?;
        head.write_all(b"ref: refs/heads/feature-branch\n")?;

        let branch = branch_for(repo_path.to_string_lossy().as_ref());
        assert_eq!(branch.as_deref(), Some("feature-branch"));
        Ok(())
    }

    #[test]
    fn branch_for_worktree_gitdir_file() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let repo_path = temp.path();

        // Create a fake worktree structure
        let worktree_git = repo_path.join("worktree-git");
        fs::create_dir_all(&worktree_git)?;
        let mut head = fs::File::create(worktree_git.join("HEAD"))?;
        head.write_all(b"ref: refs/heads/wt-branch\n")?;

        // Create the .git file that points to worktree-git
        let mut git_file = fs::File::create(repo_path.join(".git"))?;
        git_file.write_all(format!("gitdir: {}\n", worktree_git.display()).as_bytes())?;

        let branch = branch_for(repo_path.to_string_lossy().as_ref());
        assert_eq!(branch.as_deref(), Some("wt-branch"));
        Ok(())
    }

    #[test]
    fn branch_for_caches_result() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let repo_path = temp.path();
        let git_dir = repo_path.join(".git");
        fs::create_dir(&git_dir)?;
        let mut head = fs::File::create(git_dir.join("HEAD"))?;
        head.write_all(b"ref: refs/heads/cached-branch\n")?;

        let repo_str = repo_path.to_string_lossy().into_owned();

        // First call
        let branch1 = branch_for(&repo_str);
        assert_eq!(branch1.as_deref(), Some("cached-branch"));

        // Modify the file
        fs::write(git_dir.join("HEAD"), b"ref: refs/heads/changed-branch\n")?;

        // Second call should return cached value
        let branch2 = branch_for(&repo_str);
        assert_eq!(branch2.as_deref(), Some("cached-branch"));

        // Clean up cache for other tests
        invalidate();
        Ok(())
    }
}

