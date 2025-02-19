use clap::{command, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::io::{self, Read};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "CMakeForge",
    version = "0.2",
    about = "A simple CLI for build management"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize the project
    Init,
    /// Call the configure command
    Configure,
    /// Select current build target
    SelectCurrentBuild,
    /// Build the current build target
    Build,
    /// Run the current build target
    Run,
}

#[derive(Serialize, Deserialize)]
struct CacheJson {
    workspace: String,
    build_targets: Vec<String>,
    // The selected build target
    current_build_target: String,
    builds: Vec<BuildJson>,
    runs: Vec<RunJson>,
    configurations: Vec<ConfigureJson>,
}

#[derive(Serialize, Deserialize)]
struct BuildJson {
    name: String,
    command: String,
    args: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct RunJson {
    name: String,
    command: String,
    args: Vec<String>,
    pre_build: bool,
}

#[derive(Serialize, Deserialize)]
struct ConfigureJson {
    name: String,
    command: String,
    args: Vec<String>,
}

impl BuildJson {
    fn build(&self, workspace: &PathBuf) -> Result<(), Box<dyn Error>> {
        let vec_of_slices: Vec<&str> = self.args.iter().map(|s| s.as_str()).collect();
        run_command(workspace, &self.command, &vec_of_slices)
    }
}

impl RunJson {
    fn run(&self, workspace: &PathBuf) -> Result<(), Box<dyn Error>> {
        let vec_of_slices: Vec<&str> = self.args.iter().map(|s| s.as_str()).collect();
        run_command(workspace, &self.command, &vec_of_slices)
    }
}

impl ConfigureJson {
    fn configure(&self, workspace: &PathBuf) -> Result<(), Box<dyn Error>> {
        let vec_of_slices: Vec<&str> = self.args.iter().map(|s| s.as_str()).collect();
        run_command(workspace, &self.command, &vec_of_slices)
    }
}

fn run_command(workspace: &PathBuf, command: &str, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let mut child = Command::new(command)
        .args(args)
        .current_dir(workspace)
        .spawn()?;

    // Process stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            println!("{}", line?);
        }
    }

    // Process stderr
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            eprintln!("{}", line?);
        }
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(format!("Command failed with status: {}", status).into());
    }
    Ok(())
}

fn confirm_overwrite() -> bool {
    println!("File already exists. Do you want to overwrite it?[y/n](Empty 'no')");
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

/// * `workspace` - cmake project workspace
/// * `path` - path where the json should be created
/// # Panics
/// Panics if cannot read/write into ~/.cache/CMakeForge/ path
fn create_json_in_workspace(json_path: &PathBuf, workspace: &PathBuf) {
    if json_path.exists() {
        // ask the user if want to overwrite the file
        if !confirm_overwrite() {
            return;
        }
    }
    let mut file = File::create(json_path).unwrap();

    let cache = CacheJson {
        workspace: workspace.to_string_lossy().into_owned(),
        build_targets: vec!["test1".to_string(), "test2".to_string()],
        current_build_target: "test1".to_string(),
        // create builds vector with data
        builds: vec![BuildJson {
            name: "test1".to_string(),
            command: "cmake ..".to_string(),
            args: vec!["-DCMAKE_BUILD_TYPE=Debug".to_string()],
        }],
        runs: vec![
            RunJson {
                name: "test1".to_string(),
                command: "/my/super/app".to_string(),
                args: vec!["--arg1".to_string(), "--arg2".to_string()],
                pre_build: true,
            },
            RunJson {
                name: "test2".to_string(),
                command: "/my/super/app".to_string(),
                args: vec!["--arg1".to_string(), "--arg2".to_string()],
                pre_build: true,
            },
        ],
        configurations: vec![ConfigureJson {
            name: "test1".to_string(),
            command: "cmake".to_string(),
            args: vec![
                "-DCMAKE_BUILD_TYPE=Debug".to_string(),
                "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON".to_string(),
                "-G".to_string(),
                "Ninja".to_string(),
            ],
        }],
    };
    let json_string = serde_json::to_string_pretty(&cache).expect("Failed to serialize");
    println!(
        "Creating json file config for cmake in: {}",
        json_path.to_str().unwrap()
    );
    file.write_all(json_string.as_bytes()).unwrap();
}

fn select_current_build_target(json_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    if !json_path.exists() {
        return Err("Json file does not exist".into());
    }

    // Open file in read mode
    let mut file = File::open(json_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Parse JSON
    let mut cache: CacheJson = serde_json::from_str(&contents)?;

    println!("Current build target: {}", cache.current_build_target);
    for (i, target) in cache.build_targets.iter().enumerate() {
        println!("{}: {}", i, target);
    }

    let mut input = String::new();
    println!("Enter the index of the build target you want to select:");
    io::stdin().read_line(&mut input)?;

    // Parse user input
    let index: usize = input.trim().parse().map_err(|_| "Invalid input")?;

    if index >= cache.build_targets.len() {
        return Err("Invalid index".into());
    }

    // Update selected target
    cache.current_build_target = cache.build_targets[index].clone();
    println!("Selected build target: {}", cache.current_build_target);

    // Write back to file
    let json_string = serde_json::to_string_pretty(&cache)?;
    let mut file = File::create(json_path)?;
    file.write_all(json_string.as_bytes())?;

    Ok(())
}

fn build_current_target(json_path: &PathBuf, workspace: &PathBuf) -> Result<(), Box<dyn Error>> {
    if !json_path.exists() {
        return Err("No build target selected".into());
    }
    let mut json_file = File::open(json_path)?;
    let mut contents = String::new();
    json_file.read_to_string(&mut contents)?;

    let cache: CacheJson = serde_json::from_str(&contents)?;
    println!("Current build target: {}", cache.current_build_target);
    // From the 'builds' extract the build target
    for curr_build in &cache.builds {
        if curr_build.name == cache.current_build_target {
            println!("Building {}", curr_build.name);
            // Add your build logic here
            return curr_build.build(workspace);
        }
    }
    Err(format!("Build target not found: {}", cache.current_build_target).into())
}

fn run_current_target(json_path: &PathBuf, workspace: &PathBuf) -> Result<(), Box<dyn Error>> {
    let cache: CacheJson = read_cache(json_path)?;
    println!("Current run target: {}", cache.current_build_target);
    // From the 'builds' extract the build target
    for curr_run in &cache.runs {
        if curr_run.name == cache.current_build_target {
            if curr_run.pre_build {
                build_current_target(json_path, workspace)?;
            }
            println!("Running {}", curr_run.name);
            // Add your run logic here
            return curr_run.run(workspace);
        }
    }
    Err(format!("Run target not found: {}", cache.current_build_target).into())
}

fn configure_current_build_target(
    json_path: &PathBuf,
    workspace: &PathBuf,
) -> Result<(), Box<dyn Error>> {
    let cache: CacheJson = read_cache(json_path)?;
    println!("Current build target: {}", cache.current_build_target);
    for curr_config in &cache.configurations {
        if curr_config.name == cache.current_build_target {
            println!("Configuring {}", curr_config.name);
            // Add your configure logic here
            return curr_config.configure(workspace);
        }
    }
    Err(format!("Configure target not found: {}", cache.current_build_target).into())
}

fn cli_parser(workspace: &PathBuf, json_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    if json_path.exists() {
        println!("Loading json: {}", json_path.display());
    }
    match &cli.command {
        Commands::Init => {
            create_json_in_workspace(json_path, workspace);
        }
        Commands::SelectCurrentBuild => {
            select_current_build_target(json_path)?;
        }
        Commands::Build => {
            build_current_target(json_path, workspace)?;
        }
        Commands::Run => {
            run_current_target(json_path, workspace)?;
        }
        Commands::Configure => {
            configure_current_build_target(json_path, workspace)?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Constants for directory and file names
    const CACHE_DIR: &str = ".cache";
    const CMAKE_FORGE_DIR: &str = "CMakeForge";
    const JSON_EXTENSION: &str = ".json";

    // Get the current working directory
    let exe_path = env::current_dir().expect("Failed to get current directory");
    println!("Executable Path: {}", exe_path.display());

    // Get the home directory from environment variable
    let home_path: PathBuf = match env::var("HOME") {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            println!("HOME environment variable not found.");
            return Err("HOME environment variable not found.".into());
        }
    };

    // Ensure HOME directory exists
    if !home_path.exists() {
        return Err("HOME directory does not exist.".into());
    }

    // Construct cache path: ~/.cache/CMakeForge
    let cache_path = home_path.join(CACHE_DIR).join(CMAKE_FORGE_DIR);

    // Check and create the directory if it doesn't exist
    if !cache_path.exists() {
        if let Err(e) = std::fs::create_dir_all(&cache_path) {
            println!("Failed to create directory: {}", e);
            return Err("Failed to create directory.".into());
        }
        println!("Directory created: {}", cache_path.display());
    }

    // Deduce project name
    let project_name_deduced = match exe_path.file_name() {
        Some(name) => format!("{}{}", name.to_string_lossy(), JSON_EXTENSION),
        None => {
            return Err("Failed to deduce project name.".into());
        }
    };
    if project_name_deduced.is_empty() {
        return Err("Project name cannot be deduced.".into());
    }

    // Call the CLI parser
    cli_parser(&exe_path, &cache_path.join(project_name_deduced))
}

fn read_cache(json_path: &PathBuf) -> Result<CacheJson, Box<dyn Error>> {
    let mut file = File::open(json_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(serde_json::from_str(&contents)?)
}

// fn write_cache(json_path: &PathBuf, cache: &CacheJson) -> Result<(), Box<dyn Error>> {
//     let json_string = serde_json::to_string_pretty(cache)?;
//     let mut file = File::create(json_path)?;
//     file.write_all(json_string.as_bytes())?;
//     Ok(())
// }
