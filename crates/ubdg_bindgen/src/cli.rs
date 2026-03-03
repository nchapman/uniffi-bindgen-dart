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
    /// Generate Dart bindings from a UDL file or compiled cdylib.
    Generate(GenerateArgs),
    /// Check host tooling and print diagnostics.
    Doctor,
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    /// Path to a UDL file or compiled cdylib. Mode is auto-detected from the file extension.
    pub source: PathBuf,

    /// Directory in which to write generated files.
    #[arg(long)]
    pub out_dir: PathBuf,

    /// Deprecated: mode is now auto-detected from the file extension. Kept for backwards compatibility.
    #[arg(long, hide = true)]
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
    fn parses_generate_without_library_flag() {
        let args = CliArgs::parse_from([
            "uniffi-bindgen-dart",
            "generate",
            "path/to/libmycrate.so",
            "--out-dir",
            "out",
            "--crate",
            "mycrate",
        ]);

        match args.command {
            CliCommand::Generate(g) => {
                assert_eq!(g.source, PathBuf::from("path/to/libmycrate.so"));
                assert!(
                    !g.library,
                    "library flag should default to false (auto-detected)"
                );
                assert_eq!(g.crate_name.as_deref(), Some("mycrate"));
            }
            _ => panic!("expected generate command"),
        }
    }

    #[test]
    fn library_flag_still_accepted_for_backwards_compat() {
        let args = CliArgs::parse_from([
            "uniffi-bindgen-dart",
            "generate",
            "path/to/libmycrate.so",
            "--library",
            "--out-dir",
            "out",
        ]);

        match args.command {
            CliCommand::Generate(g) => {
                assert!(g.library);
            }
            _ => panic!("expected generate command"),
        }
    }

    #[test]
    fn run_generate_creates_output_file() {
        let temp_dir = tempfile::tempdir().expect("create tempdir");
        let out_dir = temp_dir.path().join("out");
        let source = temp_dir.path().join("simple-fns.udl");
        std::fs::write(&source, "namespace simple_fns { u32 noop(); };").expect("write source");

        run([
            "uniffi-bindgen-dart",
            "generate",
            source.to_str().expect("source path to str"),
            "--out-dir",
            out_dir.to_str().expect("path to str"),
        ])
        .expect("run generate");

        assert!(out_dir.exists());
        assert!(out_dir.join("simple_fns.dart").exists());
    }
}
