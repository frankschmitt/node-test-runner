extern crate clap;
extern crate num_cpus;

use std::env;
use std::io;
use std::fs;
use std::io::{Read, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Child, Stdio};
use std::collections::{HashSet, HashMap};
use read_elmi::ReadElmiError;
use abort::Abort;

mod files;
mod cli;
mod read_elmi;
mod error_messages;
mod abort;

fn main() {
    run().unwrap_or_else(report_error);
}

fn report_error(error: Abort) {
    // TODO this should be eprintln! once I upgrade the version of Rust I'm using.
    println!("Error: {}", error_messages::report(error));

    std::process::exit(1);
}

fn run() -> Result<(), Abort> {
    // Find the nearest ancestor elm.json and change to that directory.
    // This way, you can run elm-test from any child directory and have it do the right thing.
    let root = files::find_nearest_elm_json(&mut env::current_dir().map_err(Abort::InvalidCwd)?)
        .ok_or(Abort::MissingElmJson)?
        .with_file_name("");

    env::set_current_dir(&root).map_err(Abort::ChDirError)?;

    // If there are no test-dependencies in elm.json, offer to init.
    init_if_necessary();

    // Parse and validate CLI arguments
    let args = cli::parse_args().map_err(Abort::CliArgParseError)?;
    let test_files = match gather_test_files(&args.file_paths).map_err(
        Abort::ReadTestFiles,
    )? {
        Some(valid_files) => valid_files,

        None => {
            return Err(Abort::NoTestsFound(args.file_paths));
        }
    };

    let path_to_elm_binary: PathBuf = elm_binary_path_from_compiler_flag(args.compiler)?;

    // Print the headline. Something like:
    //
    // elm-test 0.18.10
    // ----------------
    print_headline();

    // Start `elm make` running.
    let elm_make_process = Command::new(path_to_elm_binary)
        .arg("make")
        .arg("--yes")
        .arg("--output=/dev/null")
        .args(&test_files)
        .spawn()
        .map_err(Abort::SpawnElmMake)?;

    // Spin up node processes in a separate thread.
    let mut node_processes: Vec<std::process::Child> = Vec::new();

    for _ in 0..num_cpus::get() {
        let node_process = Command::new("node")
            .arg("-p")
            .arg("'hi from node'")
            .spawn()
            .map_err(Abort::SpawnElmMake)?;

        node_processes.push(node_process);
    }

    let source_dirs = files::read_source_dirs(root.as_path()).map_err(
        Abort::ReadElmJson,
    )?;

    let possible_module_names = files::possible_module_names(&test_files, &source_dirs);

    for node_process in node_processes {
        node_process.wait_with_output().map_err(
            Abort::SpawnNodeProcess,
        )?;
    }

    elm_make_process.wait_with_output().map_err(
        Abort::CompilationFailed,
    )?;

    read_elmi::read_test_interfaces(root.as_path(), &possible_module_names)
        .map_err(Abort::ReadElmi)?;

    Ok(())
}

// Default to searching the tests/ directory for tests.
const DEFAULT_TEST_FILES_ARGUMENT: &str = "tests";

fn gather_test_files(values: &HashSet<PathBuf>) -> io::Result<Option<HashSet<PathBuf>>> {
    let results = &mut HashSet::new();

    // It's okay if the user didn't specify any files to run; fall back on the default choice.
    if values.is_empty() {
        files::gather_all(
            results,
            [DEFAULT_TEST_FILES_ARGUMENT].iter().map(|&str| {
                Path::new(str).to_path_buf()
            }),
        )?;
    } else {
        // TODO there is presumably a way to avoid this .clone() but I couldn't figure it out.
        files::gather_all(results, values.clone().into_iter())?;
    }

    if results.is_empty() {
        Ok(None)
    } else {
        Ok(Some(results.clone()))
    }
}

// Return the path to the elm binary
fn elm_binary_path_from_compiler_flag(compiler_flag: Option<String>) -> Result<PathBuf, Abort> {
    match compiler_flag {
        Some(string) => {
            match PathBuf::from(string.as_str()).canonicalize() {
                Ok(path_to_elm_binary) => {
                    if path_to_elm_binary.is_dir() {
                        Err(Abort::InvalidCompilerFlag(string))
                    } else {
                        Ok(path_to_elm_binary)
                    }
                }

                Err(_) => Err(Abort::InvalidCompilerFlag(string)),
            }
        }
        None => Ok(PathBuf::from("elm")),
    }
}

// prints something like this:
//
// elm-test 0.18.10
// ----------------
fn print_headline() {
    let headline = String::from("elm-test ") + cli::VERSION;
    let bar = "-".repeat(headline.len());

    println!("\n{}\n{}\n", headline, bar);
}

fn init_if_necessary() {
    // TODO check if there are any test-dependencies in elm.json.
    // If there aren't, then offer to init.
}
