use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "uniffi-bindgen-dart")]
#[command(about = "Generate Dart bindings for UniFFI components")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Generate bindings from a UDL file or library.
    Generate(GenerateArgs),
    /// Check host tooling and print diagnostics.
    Doctor,
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    /// A UDL file or library path.
    pub source: PathBuf,

    /// Directory in which to write generated files.
    #[arg(long)]
    pub out_dir: PathBuf,

    /// Treat source as a library and run in library mode.
    #[arg(long)]
    pub library: bool,

    /// Optional config file path.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Optional crate name override.
    #[arg(long = "crate")]
    pub crate_name: Option<String>,

    /// Skip formatting generated bindings.
    #[arg(long)]
    pub no_format: bool,
}

pub fn run<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = CliArgs::parse_from(args);
    match cli.command {
        CliCommand::Generate(generate_args) => crate::dart::generate_bindings(&generate_args),
        CliCommand::Doctor => {
            run_doctor();
            Ok(())
        }
    }
}

fn run_doctor() {
    let dart_ok = std::process::Command::new("dart")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if dart_ok {
        println!("dart: detected");
    } else {
        println!("dart: not detected (install Dart SDK to run host tests)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generate_flags() {
        let args = CliArgs::parse_from([
            "uniffi-bindgen-dart",
            "generate",
            "fixtures/simple-fns/src/simple-fns.udl",
            "--out-dir",
            "out",
            "--library",
            "--config",
            "uniffi.toml",
            "--crate",
            "demo",
            "--no-format",
        ]);

        match args.command {
            CliCommand::Generate(g) => {
                assert_eq!(
                    g.source,
                    PathBuf::from("fixtures/simple-fns/src/simple-fns.udl")
                );
                assert_eq!(g.out_dir, PathBuf::from("out"));
                assert!(g.library);
                assert_eq!(g.config, Some(PathBuf::from("uniffi.toml")));
                assert_eq!(g.crate_name.as_deref(), Some("demo"));
                assert!(g.no_format);
            }
            _ => panic!("expected generate command"),
        }
    }

    #[test]
    fn run_generate_creates_output_dir() {
        let temp_dir = tempfile::tempdir().expect("create tempdir");
        let out_dir = temp_dir.path().join("out");

        run([
            "uniffi-bindgen-dart",
            "generate",
            "fixtures/simple-fns/src/simple-fns.udl",
            "--out-dir",
            out_dir.to_str().expect("path to str"),
        ])
        .expect("run generate");

        assert!(out_dir.exists());
        assert!(out_dir.join(".ubdg-stub").exists());
    }
}
