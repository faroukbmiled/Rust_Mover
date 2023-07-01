use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;
use indicatif::{ProgressBar, ProgressStyle};

fn count_files(path: &Path) -> usize {
    let mut count = 0;
    let walker = WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok);

    for entry in walker {
        if entry.file_type().is_file() {
            let filename = entry.file_name().to_string_lossy().to_lowercase();
            if filename == "passwords.txt" || filename == "password.txt" {
                count += 1;
            }
        }
    }

    count
}

fn move_password_file(
    file: &Path,
    destination: &Path,
    file_counter: Arc<AtomicUsize>,
    overall_counter: Arc<AtomicUsize>,
    pb: Arc<ProgressBar>,
    ){
    let filename = file.file_name().unwrap();
    let name = filename.to_str().unwrap();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();

    let count = file_counter.fetch_add(1, Ordering::SeqCst);

    let extension = file.extension().unwrap().to_str().unwrap();

    let new_filename = format!("{}_{}_{}.{}", name, timestamp, count, extension);
    let new_destination = destination.join(new_filename);

    match fs::rename(file, &new_destination) {
        Ok(_) => {}
        Err(e) => {
            println!("Error moving file: {}", e);
        }
    }

    let overall_count = overall_counter.fetch_add(1, Ordering::SeqCst) + 1;
    pb.set_position(overall_count as u64);
}

fn search_and_move_password_files(
    path: &Path,
    destination: &Path,
    file_counter: Arc<AtomicUsize>,
    overall_counter: Arc<AtomicUsize>,
    pb: Arc<ProgressBar>,
) {
    let walker = WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok);

    for entry in walker {
        if entry.file_type().is_file() {
            let filename = entry
                .file_name()
                .to_string_lossy()
                .to_lowercase();
            if filename == "passwords.txt" || filename == "password.txt" {
                move_password_file(
                    &entry.path(),
                    destination,
                    Arc::clone(&file_counter),
                    Arc::clone(&overall_counter),
                    Arc::clone(&pb),
                );
            }
        }
    }
}

fn process_directory(path: &Path, destination: &Path) {
    if !destination.exists() {
        if let Err(e) = fs::create_dir_all(destination) {
            println!("Error creating destination folder: {}", e);
            return;
        }
    }

    let num_files = count_files(path);
    if num_files == 0 {
        println!("\x1b[31mNo files to move.\x1b[0m");
        return;
    }

    let file_counter = Arc::new(AtomicUsize::new(0));
    let overall_counter = Arc::new(AtomicUsize::new(0));

    let pb = Arc::new(ProgressBar::new(num_files as u64));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")
            .progress_chars("#>-"),
    );

    let mut join_handles = Vec::new();

    for entry in fs::read_dir(path).unwrap() {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if entry_path.is_file() {
                let filename = entry_path.file_name().unwrap().to_str().unwrap().to_lowercase();
                if filename == "passwords.txt" || filename == "password.txt" {
                    let file_counter_clone = Arc::clone(&file_counter);
                    let overall_counter_clone = Arc::clone(&overall_counter);
                    let destination_clone = destination.to_owned();
                    let pb_clone = Arc::clone(&pb);
                    join_handles.push(thread::spawn(move || {
                        move_password_file(
                            &entry_path,
                            &destination_clone,
                            file_counter_clone,
                            overall_counter_clone,
                            pb_clone,
                        );
                    }));
                }
            }
        }
    }

    for handle in join_handles {
        handle.join().unwrap();
    }

    pb.finish_with_message(&format!("Done! Files moved to: {}", destination.display()));

    search_and_move_password_files(
        path,
        destination,
        Arc::clone(&file_counter),
        Arc::clone(&overall_counter),
        Arc::clone(&pb),
    );
}

fn create_valid_config_file(config_file: &Path, path: &str, destination: &str) {
    let mut file = fs::File::create(config_file).unwrap();
    writeln!(file, "Path=\"{}\"", path).unwrap();
    writeln!(file, "DestPath=\"{}\"", destination).unwrap();
}

fn parse_config_file(config_file: &Path) -> Option<(PathBuf, PathBuf)> {
    if !config_file.exists() {
        return None;
    }

    let contents = fs::read_to_string(config_file).unwrap();
    let mut path = None;
    let mut destination = None;

    for line in contents.lines() {
        if line.starts_with("Path=") {
            let path_value = line[5..].trim_matches('"');
            path = Some(PathBuf::from(path_value));
        } else if line.starts_with("DestPath=") {
            let destination_value = line[9..].trim_matches('"');
            destination = Some(PathBuf::from(destination_value));
        }
    }

    match (path, destination) {
        (Some(p), Some(d)) => Some((p, d)),
        _ => None,
    }
}

fn main() {
    let config_file = "path.conf";
    let path: PathBuf;
    let destination: PathBuf;
    let mut path_updated = false;

    if let Some((parsed_path, parsed_destination)) = parse_config_file(Path::new(config_file)) {
        path = parsed_path;
        destination = parsed_destination;
    } else {
        println!("Config file not found or invalid. Creating a new config file.");
        print!("Enter the path: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let path_value = {
            let trimmed = input.trim();
            trimmed.to_string()
        };

        print!("Enter the destination folder path: ");
        io::stdout().flush().unwrap();
        input.clear();
        io::stdin().read_line(&mut input).unwrap();
        let destination_value = {
            let trimmed = input.trim();
            trimmed.to_string()
        };

        path = PathBuf::from(&path_value);
        destination = PathBuf::from(&destination_value);

        create_valid_config_file(Path::new(config_file), &path_value, &destination_value);
        path_updated = true;
    }

    println!("\x1b[1mMoving password files...\x1b[0m");

    if path_updated {
        println!("\x1b[33mUsing the updated path: {}\x1b[0m", path.display());
    } else {
        println!("\x1b[33mUsing the path from the config file: {}\x1b[0m", path.display());
    }

    process_directory(&path, &destination);
}
