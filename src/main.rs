use clap::{Parser, Subcommand};
use machunt::{Engine, FileEntry, SearchMode, SearchOptions, SortKey};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "machunt")]
#[command(about = "macOS Global File Search Tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search files by name (substring or wildcard pattern).
    Search {
        /// Search query
        #[arg(default_value = "")]
        query: String,

        /// Use wildcard/regex pattern mode (*.rs, test?.txt, {foo,bar})
        #[arg(short = 'p', long)]
        pattern: bool,

        /// Case-sensitive search
        #[arg(short = 'c', long)]
        case_sensitive: bool,

        /// Limit result count
        #[arg(short = 'n', long, default_value_t = 100)]
        limit: usize,

        /// Filter results under this path prefix
        #[arg(short = 'P', long)]
        path: Option<String>,

        /// Include only files
        #[arg(short, long)]
        files: bool,

        /// Include only directories
        #[arg(short, long)]
        dirs: bool,

        /// Fuzzy search (typo-tolerant, edit distance <= 1~2)
        #[arg(short = 'F', long)]
        fuzzy: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Build or rebuild the file index.
    Build {
        #[arg(short, long)]
        path: Option<String>,

        #[arg(long)]
        rebuild: bool,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        include_dirs: bool,
    },
    /// Watch for file system changes (interactive mode).
    Watch,
    /// Database maintenance (checkpoint and vacuum).
    Optimize {
        #[arg(long, default_value_t = false)]
        vacuum: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let engine = Engine::new(false);

    match cli.command {
        Commands::Search {
            query,
            pattern,
            fuzzy,
            case_sensitive,
            limit,
            path,
            files,
            dirs,
            json,
        } => {
            let query = query.trim().to_string();
            if query.is_empty() {
                eprintln!("Usage: machunt search <query>");
                std::process::exit(1);
            }

            let mode = if fuzzy {
                SearchMode::Fuzzy
            } else if pattern {
                SearchMode::Pattern
            } else {
                SearchMode::Substring
            };

            let options = SearchOptions {
                query,
                mode,
                case_sensitive,
                path_prefix: path.map(PathBuf::from),
                include_files: !dirs || files,
                include_dirs: !files || dirs,
                limit: Some(limit),
                extensions: None,
                sort_key: SortKey::default(),
                sort_ascending: true,
                size_min_bytes: None,
                size_max_bytes: None,
                time_min_ms: None,
                time_max_ms: None,
            };

            let start = Instant::now();
            let results = engine.search(options);
            let elapsed = start.elapsed();

            if json {
                print_json(&results, elapsed);
            } else {
                println!(
                    "Found {} results in {:?}",
                    results.len(),
                    elapsed
                );
                for entry in &results {
                    let full = if entry.dir_path == "/" { format!("/{}", entry.file_name) } else { format!("{}/{}", entry.dir_path, entry.file_name) };
                    println!("{}", full);
                }
            }
        }
        Commands::Build {
            path,
            rebuild,
            include_dirs,
        } => {
            engine.build_index(path, rebuild, include_dirs, true);
        }
        Commands::Watch => {
            let has_index = engine.has_persisted_index();
            let last_event_id = engine.load_last_event_id();

            if !has_index {
                println!("First run, building index in background...");
                engine.start_watch(None);
                let engine_bg = engine.clone();
                std::thread::spawn(move || {
                    engine_bg.build_index(None, true, true, true);
                });
            } else {
                match last_event_id {
                    Some(id) => {
                        let current_id = unsafe { machunt::watcher::FSEventsGetCurrentEventId() };
                        if id < current_id && current_id - id > 50_000 {
                            println!(
                                "EventID {} too far behind (gap: {}), starting background catch-up",
                                id, current_id - id
                            );
                            engine.start_watch(None);
                            let engine_bg = engine.clone();
                            std::thread::spawn(move || {
                                engine_bg.build_index(None, false, true, false);
                            });
                        } else {
                            println!(
                                "Resuming from last exit point (EventID: {}), replaying...",
                                id
                            );
                            engine.start_watch(Some(id));
                        }
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

                let options = SearchOptions {
                    query: input.to_string(),
                    mode: SearchMode::Substring,
                    case_sensitive: false,
                    path_prefix: None,
                    include_files: true,
                    include_dirs: true,
                    limit: Some(50),
                    extensions: None,
                    sort_key: SortKey::default(),
                    sort_ascending: true,
                    size_min_bytes: None,
                    size_max_bytes: None,
                    time_min_ms: None,
                    time_max_ms: None,
                };
                let results = engine.search(options);
                println!("Found {} results", results.len());
                for entry in results {
                    let full = if entry.dir_path == "/" { format!("/{}", entry.file_name) } else { format!("{}/{}", entry.dir_path, entry.file_name) };
                    println!("{}", full);
                }
            }
        }
        Commands::Optimize { vacuum } => {
            engine.checkpoint_wal();
            if vacuum {
                engine.vacuum();
                println!("WAL checkpoint + VACUUM finished");
            } else {
                println!("WAL checkpoint finished (pass --vacuum to reclaim DB file space)");
            }
        }
    }
}

fn print_json(results: &[FileEntry], elapsed: std::time::Duration) {
    let items: Vec<serde_json::Value> = results
        .iter()
        .map(|e| {
            let full_path = if e.dir_path == "/" { format!("/{}", e.file_name) } else { format!("{}/{}", e.dir_path, e.file_name) };
            serde_json::json!({
                "name": e.file_name,
                "path": full_path,
                "parent": e.dir_path,
                "size": e.size_bytes.unwrap_or(0),
                "is_dir": e.is_dir,
            })
        })
        .collect();

    let output = serde_json::json!({
        "count": results.len(),
        "took_ms": elapsed.as_millis(),
        "items": items,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
