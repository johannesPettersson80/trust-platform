//! Build the program image of a runtime bundle from its sources (specification 44 §4.1, §8.8).

use clap::Parser;
use std::path::PathBuf;

use trust_runtime::bundle_builder::build_program_image;
use trust_runtime::native::{Target, Toolchain};

#[derive(Debug, Parser)]
#[command(
    name = "trust-bundle-gen",
    about = "Build program.tpi for a runtime bundle"
)]
struct Args {
    /// Runtime bundle directory.
    #[arg(long, value_name = "DIR")]
    bundle: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // The toolchain is found, not installed: `trust-runtime toolchain install` downloads it.
    let toolchain = Toolchain::find()?;
    let target = Target::this_cpu()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU runs no program image"))?;
    let report = build_program_image(&args.bundle, None, target, &toolchain, false, true)?;
    println!("Wrote {}", report.image.display());
    Ok(())
}
