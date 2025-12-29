use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        format!("{}h {}m {}s", hours, mins, secs)
    } else if secs >= 60 {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

fn get_file_size(path: &Path) -> io::Result<u64> {
    fs::metadata(path).map(|m| m.len())
}

fn print_summary(initial_size: Option<u64>, final_size: u64, elapsed_secs: u64) {
    let total_downloaded = final_size.saturating_sub(initial_size.unwrap_or(0));

    println!("\n\n--- Download Summary ---");
    println!("Total downloaded: {}", format_bytes(total_downloaded));
    println!("Final size: {}", format_bytes(final_size));
    println!("Duration: {}", format_duration(elapsed_secs));

    if elapsed_secs > 0 {
        let avg_speed = total_downloaded / elapsed_secs;
        println!("Average speed: {}/s", format_bytes(avg_speed));
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        std::process::exit(1);
    }

    let file_path = Path::new(&args[1]);

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    println!("Monitoring download speed for: {}", file_path.display());
    println!("Press Ctrl+C to stop (auto-exits after 5s of no activity)\n");

    let mut previous_size: Option<u64> = None;
    let mut initial_size: Option<u64> = None;
    let mut last_known_size: u64 = 0;
    let mut no_change_count = 0;
    let start_time = Instant::now();

    while running.load(Ordering::SeqCst) {
        match get_file_size(file_path) {
            Ok(current_size) => {
                last_known_size = current_size;

                if initial_size.is_none() {
                    initial_size = Some(current_size);
                }

                if let Some(prev) = previous_size {
                    let delta = current_size.saturating_sub(prev);

                    if delta == 0 {
                        no_change_count += 1;
                        print!(
                            "\rSize: {} | Speed: 0 B/s (idle {}/5)    ",
                            format_bytes(current_size),
                            no_change_count
                        );
                    } else {
                        no_change_count = 0;
                        print!(
                            "\rSize: {} | Speed: {}/s         ",
                            format_bytes(current_size),
                            format_bytes(delta)
                        );
                    }
                    io::stdout().flush().unwrap();

                    if no_change_count >= 5 {
                        print_summary(initial_size, current_size, start_time.elapsed().as_secs());
                        return;
                    }
                } else {
                    println!("Initial size: {}", format_bytes(current_size));
                }
                previous_size = Some(current_size);
            }
            Err(e) => {
                if previous_size.is_some() {
                    println!("\nFile no longer accessible: {}", e);
                    print_summary(initial_size, last_known_size, start_time.elapsed().as_secs());
                    return;
                } else {
                    print!("\rWaiting for file to appear...    ");
                    io::stdout().flush().unwrap();
                }
            }
        }

        thread::sleep(Duration::from_secs(1));
    }

    // Ctrl+C was pressed
    print_summary(initial_size, last_known_size, start_time.elapsed().as_secs());
}
