use clap::{Parser, Subcommand};
use machunt::{Engine, SearchMode, SearchOptions};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "machunt")]
#[command(about = "macOS Global File Search Tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value = ".")]
    path: String,

    #[arg(short, long)]
    regex: bool,

    #[arg(long)]
    folder: bool,

    #[arg(long)]
    file: bool,

    #[arg(long)]
    logs: bool,

    #[arg(default_value = "")]
    query: String,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        #[arg(short, long)]
        path: Option<String>,

        #[arg(long)]
        rebuild: bool,
    },
    Watch,
}

fn search_once(engine: &Engine, cli: &Cli) {
    if cli.query.is_empty() {
        eprintln!("Error: missing query");
        std::process::exit(1);
    }

    let start = Instant::now();
    let options = build_search_options(cli, cli.query.clone());

    let results = engine.search(options);
    let duration = start.elapsed();

    println!(
        "Search completed, found {} matching files, took {:?}",
        results.len(),
        duration
    );
    for path in results {
        println!("{}", path.display());
    }
}

fn build_search_options(cli: &Cli, query: String) -> SearchOptions {
    SearchOptions {
        query,
        mode: if cli.regex {
            SearchMode::Pattern
        } else {
            SearchMode::Substring
        },
        case_sensitive: false,
        path_prefix: if cli.path != "." {
            Some(PathBuf::from(&cli.path))
        } else {
            None
        },
        include_files: cli.file,
        include_dirs: cli.folder,
        limit: None,
    }
}

fn real_time_search(engine: Engine, cli: &Cli) {
    println!("Real-time search mode, enter search term (Ctrl+C to exit):");
    loop {
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            continue;
        }
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let options = build_search_options(cli, input.to_string());
        let results = engine.search(options);
        println!("Found {} results", results.len());
        for path in results {
            println!("{}", path.display());
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let engine = Engine::new(cli.logs);

    match cli.command {
        Some(Commands::Build { path, rebuild }) => {
            engine.build_index(path, rebuild);
        }
        Some(Commands::Watch) => {
            let has_index = engine.load_index_from_db() > 0;
            let last_event_id = engine.load_last_event_id();

            if !has_index {
                println!("First run, building index in background...");
                engine.start_watch(None);
                let engine_bg = engine.clone();
                std::thread::spawn(move || {
                    engine_bg.build_index(None, true);
                });
            } else {
                match last_event_id {
                    Some(id) => {
                        println!(
                            "Resuming from last exit point (EventID: {}), playing back offline changes...",
                            id
                        );
                        engine.start_watch(Some(id));
                    }
                    None => {
                        println!("Background validation...");
                        engine.start_watch(None);
                        engine.cleanup_dead_paths_background();
                    }
                }
            }

            let engine_ctrlc = engine.clone();
            ctrlc::set_handler(move || {
                engine_ctrlc.save_last_event_id_from_runtime();
                std::process::exit(0);
            })
            .unwrap();

            real_time_search(engine, &cli);
        }
        None => {
            if engine.load_index_from_db() == 0 {
                eprintln!("Error: Index not found, please run machunt build first");
                std::process::exit(1);
            }
            search_once(&engine, &cli);
        }
    }
}
