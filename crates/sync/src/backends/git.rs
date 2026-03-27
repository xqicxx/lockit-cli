//! Git repository sync backend (GitHub, Gitee, self-hosted).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::backend::{SyncBackend, SyncMetadata};
use crate::config::GitConfig;
use crate::error::{Error, Result};

pub struct GitBackend {
    local_path: PathBuf,
    remote_url: String,
    branch: String,
}

impl GitBackend {
    pub fn new(cfg: &GitConfig) -> Result<Self> {
        let home =
            home::home_dir().ok_or_else(|| Error::Config("home directory not found".into()))?;
        let local_path = home.join(".lockit").join("git-sync");

        let backend = Self {
            local_path,
            remote_url: cfg.repo_url.clone(),
            branch: cfg.branch.clone(),
        };

        // Clone or open the repository
        backend.ensure_repo()?;

        Ok(backend)
    }

    fn ensure_repo(&self) -> Result<()> {
        if self.local_path.join(".git").exists() {
            // Already cloned
            return Ok(());
        }
        std::fs::create_dir_all(&self.local_path)
            .map_err(|e| Error::Config(format!("cannot create git-sync dir: {e}")))?;

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(ssh_agent_credentials);

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);
        builder.branch(&self.branch);

        builder
            .clone(&self.remote_url, &self.local_path)
            .map_err(|e| Error::Config(format!("git clone failed: {e}")))?;

        Ok(())
    }

    fn open_repo(&self) -> Result<git2::Repository> {
        git2::Repository::open(&self.local_path)
            .map_err(|e| Error::Config(format!("cannot open git repo: {e}")))
    }

    fn commit_and_push(&self, repo: &git2::Repository, message: &str) -> Result<()> {
        let sig =
        // Use a no-reply address so commits are accepted by all remote hosts
        // (some reject bare hostnames like "lockit@localhost").
        git2::Signature::now("lockit", "lockit@users.noreply.github.com").map_err(|e| Error::Upload {
                key: String::new(),
                reason: e.to_string(),
            })?;

        let mut index = repo.index().map_err(|e| Error::Upload {
            key: String::new(),
            reason: e.to_string(),
        })?;
        let tree_id = index.write_tree().map_err(|e| Error::Upload {
            key: String::new(),
            reason: e.to_string(),
        })?;
        let tree = repo.find_tree(tree_id).map_err(|e| Error::Upload {
            key: String::new(),
            reason: e.to_string(),
        })?;

        // Find parent commit (HEAD)
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .map_err(|e| Error::Upload {
                key: String::new(),
                reason: e.to_string(),
            })?;

        // Push
        let mut remote = repo.find_remote("origin").map_err(|e| Error::Upload {
            key: String::new(),
            reason: e.to_string(),
        })?;

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(ssh_agent_credentials);

        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        let refspec = format!("refs/heads/{}:refs/heads/{}", self.branch, self.branch);
        remote
            .push(&[&refspec], Some(&mut push_options))
            .map_err(|e| Error::Upload {
                key: String::new(),
                reason: format!("git push failed: {e}"),
            })?;

        Ok(())
    }

    fn fetch_remote(&self, repo: &git2::Repository) -> Result<()> {
        let mut remote = repo.find_remote("origin").map_err(|e| Error::Download {
            key: String::new(),
            reason: e.to_string(),
        })?;

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(ssh_agent_credentials);

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        remote
            .fetch(&[&self.branch], Some(&mut fetch_options), None)
            .map_err(|e| Error::Download {
                key: String::new(),
                reason: format!("git fetch failed: {e}"),
            })?;

        Ok(())
    }
}

fn ssh_agent_credentials(
    _url: &str,
    username: Option<&str>,
    _allowed: git2::CredentialType,
) -> std::result::Result<git2::Cred, git2::Error> {
    git2::Cred::ssh_key_from_agent(username.unwrap_or("git"))
}

#[async_trait]
impl SyncBackend for GitBackend {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        let file_path = self.local_path.join(key);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Upload {
                key: key.to_string(),
                reason: e.to_string(),
            })?;
        }
        std::fs::write(&file_path, data).map_err(|e| Error::Upload {
            key: key.to_string(),
            reason: e.to_string(),
        })?;

        let repo = self.open_repo()?;
        let mut index = repo.index().map_err(|e| Error::Upload {
            key: key.to_string(),
            reason: e.to_string(),
        })?;
        index.add_path(Path::new(key)).map_err(|e| Error::Upload {
            key: key.to_string(),
            reason: format!("git add failed: {e}"),
        })?;
        index.write().map_err(|e| Error::Upload {
            key: key.to_string(),
            reason: e.to_string(),
        })?;

        self.commit_and_push(&repo, "lockit sync")
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let repo = self.open_repo()?;
        self.fetch_remote(&repo)?;

        let file_path = self.local_path.join(key);
        std::fs::read(&file_path).map_err(|_| Error::NotFound {
            key: key.to_string(),
        })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let repo = self.open_repo()?;

        let head = repo
            .head()
            .and_then(|h| h.peel_to_tree())
            .map_err(|e| Error::List {
                prefix: prefix.to_string(),
                reason: e.to_string(),
            })?;

        let mut keys = Vec::new();
        head.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(git2::ObjectType::Blob) {
                let name = entry.name().unwrap_or_default();
                let full = if dir.is_empty() {
                    name.to_string()
                } else {
                    format!("{dir}{name}")
                };
                if !full.is_empty() && full.starts_with(prefix) {
                    keys.push(full);
                }
            }
            git2::TreeWalkResult::Ok
        })
        .map_err(|e| Error::List {
            prefix: prefix.to_string(),
            reason: e.to_string(),
        })?;

        keys.sort();
        Ok(keys)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let file_path = self.local_path.join(key);
        if file_path.exists() {
            std::fs::remove_file(&file_path).map_err(|e| Error::Delete {
                key: key.to_string(),
                reason: e.to_string(),
            })?;
        }

        let repo = self.open_repo()?;
        let mut index = repo.index().map_err(|e| Error::Delete {
            key: key.to_string(),
            reason: e.to_string(),
        })?;
        index
            .remove_path(Path::new(key))
            .map_err(|e| Error::Delete {
                key: key.to_string(),
                reason: format!("git rm failed: {e}"),
            })?;
        index.write().map_err(|e| Error::Delete {
            key: key.to_string(),
            reason: e.to_string(),
        })?;

        self.commit_and_push(&repo, &format!("lockit sync: delete {key}"))
    }

    async fn metadata(&self, key: &str) -> Result<SyncMetadata> {
        let file_path = self.local_path.join(key);
        let data = std::fs::read(&file_path).map_err(|_| Error::NotFound {
            key: key.to_string(),
        })?;
        let size = data.len() as u64;

        let mut h = Sha256::new();
        h.update(&data);
        let checksum = hex::encode(h.finalize());

        // Get last commit time for this file
        let repo = self.open_repo()?;
        let mut revwalk = repo.revwalk().map_err(|e| Error::Metadata {
            key: key.to_string(),
            reason: e.to_string(),
        })?;
        revwalk.push_head().map_err(|e| Error::Metadata {
            key: key.to_string(),
            reason: e.to_string(),
        })?;

        let mut last_modified: Option<u64> = None;
        for oid in revwalk.flatten() {
            if let Ok(commit) = repo.find_commit(oid) {
                let tree = commit.tree().ok();
                let has_file = tree
                    .map(|t| t.get_path(Path::new(key)).is_ok())
                    .unwrap_or(false);
                if has_file {
                    last_modified = Some(commit.time().seconds().max(0) as u64);
                    break;
                }
            }
        }

        Ok(SyncMetadata {
            version: 1,
            last_modified: last_modified.unwrap_or(0),
            checksum,
            size,
        })
    }

    fn backend_name(&self) -> &str {
        "git"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_name_is_git() {
        // We can't easily test GitBackend::new() without a real remote,
        // but we can test the name via a manual construction.
        // This test validates the struct fields are set correctly.
        let backend = GitBackend {
            local_path: PathBuf::from("/tmp/test"),
            remote_url: "https://github.com/example/repo.git".into(),
            branch: "main".into(),
        };
        assert_eq!(backend.backend_name(), "git");
    }
}
