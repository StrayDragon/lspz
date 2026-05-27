//! Claude Code integration for lspz.
//!
//! Provides `lspz init` to register the MCP server and inject context files.

pub mod awareness;
pub mod constants;
pub mod languages;
pub mod settings;

use std::process::ExitCode;

use self::awareness::{claude_md_has_ref, lspz_md_exists, patch_claude_md_ref, remove_claude_md_ref, remove_lspz_md, write_lspz_md};
use self::settings::{patch_mcp_server, remove_mcp_server, resolve_binary_path, settings_path, is_mcp_registered, PatchResult};

/// Run the `lspz init` command.
pub fn run(
    global: bool,
    auto_patch: bool,
    no_patch: bool,
    show: bool,
    uninstall: bool,
    dry_run: bool,
) -> ExitCode {
    if show {
        return show_config(global);
    }
    if uninstall {
        return run_uninstall(global, dry_run);
    }

    run_init(global, auto_patch, no_patch, dry_run)
}

fn run_init(global: bool, _auto_patch: bool, no_patch: bool, dry_run: bool) -> ExitCode {
    let binary_path = match resolve_binary_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let prefix = if dry_run { "[dry-run] " } else { "" };

    // 1. Write LSPZ.md
    match write_lspz_md(global, dry_run) {
        Ok(true) => println!("{prefix}Wrote LSPZ.md"),
        Ok(false) => println!("LSPZ.md already up to date"),
        Err(e) => {
            eprintln!("Error writing LSPZ.md: {e}");
            return ExitCode::FAILURE;
        }
    }

    // 2. Patch CLAUDE.md with @LSPZ.md reference
    match patch_claude_md_ref(global, dry_run) {
        Ok(true) => println!("{prefix}Added @LSPZ.md to CLAUDE.md"),
        Ok(false) => println!("CLAUDE.md already contains @LSPZ.md"),
        Err(e) => {
            eprintln!("Error patching CLAUDE.md: {e}");
            return ExitCode::FAILURE;
        }
    }

    // 3. Register MCP server in settings.json
    if no_patch {
        println!(
            "\nTo register manually, add to settings.json:\n  {}",
            format_mcp_entry(&binary_path)
        );
    } else {
        let sp = match settings_path(global) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        };

        match patch_mcp_server(&sp, &binary_path, dry_run) {
            Ok(PatchResult::Patched) => println!("{prefix}Registered MCP server in {}", sp.display()),
            Ok(PatchResult::AlreadyPresent) => println!("MCP server already registered"),
            Ok(PatchResult::WouldPatch) => {
                println!("[dry-run] Would register MCP server in {}", sp.display())
            }
            Err(e) => {
                eprintln!("Error patching settings.json: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    if dry_run {
        println!("\n[dry-run] Nothing written.");
    }

    ExitCode::SUCCESS
}

fn run_uninstall(global: bool, dry_run: bool) -> ExitCode {
    let prefix = if dry_run { "[dry-run] " } else { "" };

    // 1. Remove LSPZ.md
    match remove_lspz_md(global, dry_run) {
        Ok(true) => println!("{prefix}Removed LSPZ.md"),
        Ok(false) => println!("LSPZ.md not found"),
        Err(e) => {
            eprintln!("Error removing LSPZ.md: {e}");
            return ExitCode::FAILURE;
        }
    }

    // 2. Remove @LSPZ.md reference from CLAUDE.md
    match remove_claude_md_ref(global, dry_run) {
        Ok(true) => println!("{prefix}Removed @LSPZ.md from CLAUDE.md"),
        Ok(false) => println!("@LSPZ.md reference not found in CLAUDE.md"),
        Err(e) => {
            eprintln!("Error patching CLAUDE.md: {e}");
            return ExitCode::FAILURE;
        }
    }

    // 3. Remove MCP server entry from settings.json
    let sp = match settings_path(global) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::FAILURE;
        }
    };

    match remove_mcp_server(&sp, dry_run) {
        Ok(true) => println!("{prefix}Removed MCP server from {}", sp.display()),
        Ok(false) => println!("MCP server entry not found in {}", sp.display()),
        Err(e) => {
            eprintln!("Error removing MCP server: {e}");
            return ExitCode::FAILURE;
        }
    }

    if dry_run {
        println!("\n[dry-run] Nothing written.");
    }

    ExitCode::SUCCESS
}

fn show_config(global: bool) -> ExitCode {
    let binary_path = match resolve_binary_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let sp = match settings_path(global) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let scope = if global { "global" } else { "project" };
    let mcp_ok = is_mcp_registered(&sp, &binary_path);
    let lspz_ok = lspz_md_exists(global);
    let ref_ok = claude_md_has_ref(global);

    println!("lspz Configuration ({scope}):\n");
    println!("  Binary:    {}", binary_path.display());
    println!(
        "  MCP:       {} {}",
        status_icon(mcp_ok),
        sp.display()
    );
    println!(
        "  LSPZ.md:   {}",
        status_icon(lspz_ok)
    );
    println!(
        "  CLAUDE.md: {}",
        status_icon(ref_ok)
    );

    if !mcp_ok {
        println!(
            "\nTo register, add to {}:\n  {}",
            sp.display(),
            format_mcp_entry(&binary_path)
        );
    }

    ExitCode::SUCCESS
}

fn status_icon(ok: bool) -> &'static str {
    if ok {
        "[ok]"
    } else {
        "[missing]"
    }
}

fn format_mcp_entry(binary_path: &std::path::Path) -> String {
    format!(
        r#"{{"mcpServers": {{"lspz": {{"command": "{}", "args": ["mcp"]}}}}}}"#,
        binary_path.display()
    )
}
