// Modern Industrialization is one file on the registry: its block, models, textures, lang and loot
// are the content mod in this bundle's data/ (content/modern_industrialization.hmod, built by
// `cargo horizon-mod build`). It is written out once and loaded as a dev mod is, so it is on
// whenever this native mod is built in. An installed copy of the mod wins, as the first read.
fn modern_industrialization_embedded_content(
    root: &std::path::Path,
    enabled: &std::collections::BTreeSet<String>,
    dev: &[std::path::PathBuf],
) -> core::ops::ControlFlow<std::path::PathBuf> {
    static CONTENT: &[u8] = horizon_client::native_data!(
        "modern_industrialization",
        "modern_industrialization-content.hmod"
    );
    let dir = std::env::current_dir()
        .unwrap_or_default()
        .join(".horizon-cache");
    let file = dir.join("modern_industrialization-embedded-content.hmod");
    if dev.contains(&file) {
        return core::ops::ControlFlow::Continue(());
    }
    let same = std::fs::read(&file).is_ok_and(|on_disk| on_disk == CONTENT);
    if !same
        && std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&file, CONTENT))
            .is_err()
    {
        log::warn!(
            "Horizon: Modern Industrialization's content could not be written to {}",
            file.display()
        );
        return core::ops::ControlFlow::Continue(());
    }
    let mut with = dev.to_vec();
    with.push(file);
    core::ops::ControlFlow::Break(content_root(root, enabled, &with))
}
