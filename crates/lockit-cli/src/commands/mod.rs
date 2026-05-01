pub mod add;
pub mod init;
pub mod delete;
pub mod list;
pub mod show;
pub mod edit;
pub mod reveal;
pub mod coding_plan;
pub mod sync_cmd;
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
