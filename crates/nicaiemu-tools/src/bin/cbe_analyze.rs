//! CBE File Analyzer
//!
//! Analyzes CBE (Cool Bar Engine) game archives and displays detailed information.

use std::path::PathBuf;
use anyhow::{Context, Result};
use clap::Parser;
use nicaiemu_core::{CbeArchive, ResourceType};

/// CBE File Analyzer
#[derive(Parser)]
#[command(name = "cbe-analyze")]
#[command(about = "Analyze CBE (Cool Bar Engine) game archives")]
struct Cli {
    /// Path to the CBE file to analyze
    file: PathBuf,

    /// Show detailed resource information
    #[arg(short, long)]
    detailed: bool,

    /// Show only specific resource type (scene, map, actor, script, image)
    #[arg(short, long)]
    filter: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load the CBE archive
    let archive = CbeArchive::load(&cli.file)
        .with_context(|| format!("Failed to load CBE file: {}", cli.file.display()))?;

    // Print archive summary
    let summary = archive.summary();
    println!("\n{}", summary);

    // Filter resources by type if specified
    let filter_type = cli.filter.as_deref().and_then(|f| match f.to_lowercase().as_str() {
        "scene" => Some(ResourceType::Scene),
        "map" => Some(ResourceType::Map),
        "actor" => Some(ResourceType::Actor),
        "script" | "xse" => Some(ResourceType::Script),
        "image" | "gif" => Some(ResourceType::Image),
        "audio" => Some(ResourceType::Audio),
        _ => {
            eprintln!("Unknown resource type: {}", f);
            None
        }
    });

    // List resources
    let resources: Vec<_> = if let Some(filter) = filter_type {
        archive.resources_by_type(filter).into_iter().collect()
    } else {
        archive.resources().iter().collect()
    };

    println!("\nResources ({} total):", resources.len());
    println!("{:-<80}", "");

    for (i, resource) in resources.iter().enumerate() {
        if cli.detailed {
            println!("{:3}. [{:8}] {} (offset=0x{:06X}, size={})",
                     i + 1,
                     resource.resource_type,
                     resource.name,
                     resource.offset,
                     resource.size);
        } else {
            println!("{:3}. {}", i + 1, resource.name);
        }
    }

    // Show section information
    println!("\nSections:");
    println!("{:-<80}", "");
    for section in archive.sections() {
        let header = &section.header;
        println!("  Section {}: offset=0x{:06X}, resources={}, dataRel=0x{:06X}, dataLen=0x{:06X}",
                 header.index,
                 header.file_offset,
                 header.resource_count,
                 header.data_rel,
                 header.data_len);
    }

    // Show resource type breakdown
    println!("\nResource Type Breakdown:");
    println!("{:-<80}", "");
    let mut type_counts = std::collections::HashMap::new();
    for resource in archive.resources() {
        *type_counts.entry(resource.resource_type).or_insert(0) += 1;
    }
    for (rtype, count) in &type_counts {
        println!("  {:8}: {}", rtype, count);
    }

    Ok(())
}
