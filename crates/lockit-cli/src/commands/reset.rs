use lockit_core::vault::VaultPaths;

use crate::output;

pub fn run(paths: &VaultPaths) -> anyhow::Result<()> {
    if !paths.vault_path.exists() {
        anyhow::bail!("No vault found. Nothing to reset.");
    }

    println!("This will permanently delete your vault and all credentials.");
    println!("Type 'yes' to confirm:");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim() != "yes" {
        println!("Cancelled.");
        return Ok(());
    }

    std::fs::remove_file(&paths.vault_path)?;

    // Also clean up sync state and backup if they exist
    let backup = paths.vault_path.with_extension("lockit.pull_backup");
    if backup.exists() {
        let _ = std::fs::remove_file(&backup);
    }
    let sync_state = paths.vault_path.with_file_name("sync_state.json");
    if sync_state.exists() {
        let _ = std::fs::remove_file(&sync_state);
    }

    output::success("Vault deleted. Run 'lockit init' to create a new one.");
    Ok(())
}
