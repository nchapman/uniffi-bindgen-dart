use std::fs;

use anyhow::Result;

use crate::GenerateArgs;

pub mod callback_interface;
pub mod compounds;
pub mod config;
pub mod custom;
pub mod enum_;
pub mod error;
pub mod object;
pub mod oracle;
pub mod primitives;
pub mod record;

/// Temporary scaffold implementation for generator wiring.
pub fn generate_bindings(args: &GenerateArgs) -> Result<()> {
    fs::create_dir_all(&args.out_dir)?;
    fs::write(args.out_dir.join(".ubdg-stub"), "dart backend scaffold\n")?;
    Ok(())
}
