pub mod add;
pub mod init;
pub mod delete;

// Placeholders — implemented in later tasks
pub mod list {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _json: bool,
        _query: Option<String>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod show {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _name_or_id: &str,
        _json: bool,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod edit {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _name_or_id: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod reveal {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _name_or_id: &str,
        _field: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod coding_plan {
    pub fn list(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
    pub fn refresh(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _provider: Option<String>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod sync_cmd {
    pub fn status(
        _paths: &lockit_core::vault::VaultPaths,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
    pub fn push(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
    pub fn pull(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
    pub fn config(
        _paths: &lockit_core::vault::VaultPaths,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod env_cmd {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _name: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod run_cmd {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _name: &str,
        _cmd: &[String],
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod export_cmd {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _name: Option<String>,
        _json: bool,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
pub mod import_cmd {
    pub fn run(
        _paths: &lockit_core::vault::VaultPaths,
        _pw: Option<String>,
        _file: &std::path::PathBuf,
    ) -> anyhow::Result<()> {
        anyhow::bail!("not implemented")
    }
}
