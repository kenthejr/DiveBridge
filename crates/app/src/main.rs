//! DiveBridge MVP CLI.
//!
//! Wires the pipeline end-to-end for the Phase-2 vertical slice:
//!   UDDF file -> core::Dive -> SSI create-form mapping -> (dry-run | submit).
//!
//! Becomes the Tauri v2 backend later; for now it's a thin clap CLI so the whole
//! chain can be exercised from the terminal. `submit` is the only command that
//! touches the network, and only with explicit `--phpsessid` + `--yes`.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};

use divebridge_core::{Dive, Meters};
use divebridge_ingest_file as ingest;
use divebridge_ssi_api as ssi;

#[derive(Parser)]
#[command(name = "divebridge", about = "DiveBridge MVP: UDDF -> SSI dive-log mapping")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse a UDDF export and print a summary of each dive.
    Inspect {
        /// Path to a UDDF export.
        uddf: PathBuf,
    },
    /// Build the SSI create-dive form and print it. No network.
    DryRun(SubmitArgs),
    /// Search SSI dive sites near a coordinate (network, no auth).
    Sites {
        lat: f64,
        lon: f64,
        /// Half-span of the search box, in degrees.
        #[arg(long, default_value_t = 0.08)]
        span: f64,
    },
    /// Actually POST the dive to SSI. Requires --phpsessid and --yes.
    Submit(SubmitArgs),
}

#[derive(Args)]
struct SubmitArgs {
    /// Path to a UDDF export.
    uddf: PathBuf,
    /// SSI account user_master_id.
    #[arg(long)]
    user_master_id: String,
    /// SSI logbook dive number to assign (your SSI sequence, not the computer's).
    #[arg(long)]
    dive_nr: u32,
    /// Which dive in the file to use (0-based) when it holds several.
    #[arg(long, default_value_t = 0)]
    index: usize,
    /// Resolved SSI dive-site id (the `f` from `sites`).
    #[arg(long)]
    site_id: Option<String>,
    /// Body of water: fresh | salt.
    #[arg(long)]
    bow: Option<String>,
    #[arg(long)]
    divetype_id: Option<String>,
    #[arg(long)]
    entry_id: Option<String>,
    #[arg(long)]
    water_body_id: Option<String>,
    #[arg(long)]
    watertype_id: Option<String>,
    #[arg(long)]
    current_id: Option<String>,
    #[arg(long)]
    surface_id: Option<String>,
    #[arg(long)]
    weather_id: Option<String>,
    #[arg(long)]
    tanktype_id: Option<String>,
    #[arg(long)]
    gearconfig_id: Option<String>,
    /// Special-dive tag ids (comma-separated).
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
    /// Buddy ids (comma-separated).
    #[arg(long, value_delimiter = ',')]
    buddy_ids: Vec<String>,
    /// Session cookie (required by `submit`).
    #[arg(long)]
    phpsessid: Option<String>,
    /// Explicit confirmation required by `submit`.
    #[arg(long, default_value_t = false)]
    yes: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Inspect { uddf } => inspect(&uddf),
        Cmd::DryRun(args) => dry_run(&args),
        Cmd::Sites { lat, lon, span } => sites(lat, lon, span).await,
        Cmd::Submit(args) => submit(&args).await,
    }
}

/// Load a UDDF file and return its parsed dives.
fn load_dives(uddf: &PathBuf) -> Result<Vec<Dive>> {
    let bytes = std::fs::read(uddf).with_context(|| format!("reading {}", uddf.display()))?;
    let sources = ingest::parse_uddf(&bytes).context("parsing UDDF")?;
    Ok(ingest::to_dives(sources))
}

fn inspect(uddf: &PathBuf) -> Result<()> {
    let dives = load_dives(uddf)?;
    println!("{} dive(s) in {}\n", dives.len(), uddf.display());
    for (i, d) in dives.iter().enumerate() {
        let s = &d.summary;
        let dev = d.primary().map(|p| &p.device);
        println!("[{i}] {:?}", d.tracking);
        if let Some(dev) = dev {
            println!("    device : {} {} (sn {})", dev.make, dev.model, dev.serial);
        }
        if let Some(n) = d.primary().and_then(|p| p.computer_dive_number) {
            println!("    comp # : {n}");
        }
        println!("    start  : {}", s.start.to_rfc3339());
        println!(
            "    profile: max {:.1} m, avg {} m, bottom {:.0} min, {} descent(s)",
            s.max_depth.0,
            s.avg_depth.map(|m| format!("{:.1}", m.0)).unwrap_or_else(|| "—".into()),
            s.total_bottom_time.minutes(),
            s.descent_count,
        );
        if let (Some(b), Some(e)) = (s.pressure_start, s.pressure_end) {
            println!("    tank   : {:.0} -> {:.0} bar", b.0, e.0);
        }
        let gases: Vec<String> = s
            .gases
            .iter()
            .map(|g| if g.is_air() { "Air".into() } else { format!("EAN{:.0}", g.o2_percent) })
            .collect();
        if !gases.is_empty() {
            println!("    gas    : {}", gases.join(", "));
        }
        println!();
    }
    Ok(())
}

/// Pick the requested dive and build a SubmitContext from CLI args.
fn prepare<'a>(args: &SubmitArgs, dives: &'a [Dive]) -> Result<(&'a Dive, ssi::SubmitContext)> {
    let dive = dives
        .get(args.index)
        .ok_or_else(|| anyhow!("no dive at index {} (file has {})", args.index, dives.len()))?;
    let ctx = ssi::SubmitContext {
        user_master_id: args.user_master_id.clone(),
        dive_nr: args.dive_nr,
        dive_sites_id: args.site_id.clone(),
        dive_site_bow: args.bow.clone(),
        var_divetype_id: args.divetype_id.clone(),
        var_entry_id: args.entry_id.clone(),
        var_water_body_id: args.water_body_id.clone(),
        var_watertype_id: args.watertype_id.clone(),
        var_current_id: args.current_id.clone(),
        var_surface_id: args.surface_id.clone(),
        var_weather_id: args.weather_id.clone(),
        var_tanktype_id: args.tanktype_id.clone(),
        gearconfiguration_id: args.gearconfig_id.clone(),
        specialdive_ids: args.tags.clone(),
        buddy_ids: args.buddy_ids.clone(),
    };
    Ok((dive, ctx))
}

fn dry_run(args: &SubmitArgs) -> Result<()> {
    let dives = load_dives(&args.uddf)?;
    let (dive, ctx) = prepare(args, &dives)?;
    let fields = ssi::build_create_form(dive, &ctx).map_err(|e| anyhow!(e))?;

    println!("Populated fields (non-empty):");
    for (k, v) in fields.iter().filter(|(_, v)| !v.is_empty()) {
        println!("  {k} = {v}");
    }
    println!("\nEncoded body ({} fields total):", fields.len());
    println!("{}", ssi::encode_form(&fields));
    println!("\n(dry run — nothing sent. Use `submit` with --phpsessid --yes to POST.)");
    Ok(())
}

async fn sites(lat: f64, lon: f64, span: f64) -> Result<()> {
    let client = ssi::SsiClient::new().map_err(|e| anyhow!(e))?;
    let markers = client.search_dive_sites(lat, lon, span).await.map_err(|e| anyhow!(e))?;
    if markers.is_empty() {
        println!("No SSI dive sites within {span}° of {lat},{lon}. Widen --span or submit site-blank.");
        return Ok(());
    }
    println!("{} site(s) near {lat},{lon}:", markers.len());
    for m in &markers {
        println!("  id {:<10} {:<32} ({:.4},{:.4})", m.id, m.name, m.lat, m.lon);
    }
    Ok(())
}

async fn submit(args: &SubmitArgs) -> Result<()> {
    let sid = args
        .phpsessid
        .as_deref()
        .ok_or_else(|| anyhow!("submit requires --phpsessid <session cookie>"))?;
    if !args.yes {
        return Err(anyhow!("refusing to POST without --yes (run `dry-run` first to review)"));
    }
    let dives = load_dives(&args.uddf)?;
    let (dive, ctx) = prepare(args, &dives)?;
    let fields = ssi::build_create_form(dive, &ctx).map_err(|e| anyhow!(e))?;

    let depth = dive.summary.max_depth;
    println!(
        "Submitting dive_nr {} (max {:.1} m) to SSI as user {} ...",
        args.dive_nr,
        depth_m(depth),
        args.user_master_id
    );
    let client = ssi::SsiClient::with_phpsessid(sid).map_err(|e| anyhow!(e))?;
    let outcome = client.create_dive(&fields).await.map_err(|e| anyhow!(e))?;
    println!("-> HTTP {} (body {} bytes)", outcome.status, outcome.body_len);
    println!("Verify in your SSI logbook; delete the test dive if needed.");
    Ok(())
}

fn depth_m(m: Meters) -> f64 {
    m.0
}
