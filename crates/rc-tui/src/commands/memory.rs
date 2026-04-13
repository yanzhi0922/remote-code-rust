use std::fs;

use rc_config::RuntimeConfig;

pub fn render(config: &RuntimeConfig) {
    let global = config.paths.profile_dir.join("RC.md");
    let project = config.cwd.join(".remote-code-rust").join("RC.md");

    println!("Memory surface:");
    print_entry("global", &global);
    print_entry("project", &project);
}

fn print_entry(label: &str, path: &std::path::Path) {
    let exists = path.exists();
    let size_bytes = fs::metadata(path)
        .map(|meta| meta.len())
        .unwrap_or_default();
    println!(
        "  {label:<8} {} ({}, {} bytes)",
        path.display(),
        if exists { "present" } else { "missing" },
        size_bytes
    );
}
