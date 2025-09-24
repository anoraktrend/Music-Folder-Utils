use anyhow::Result;
use std::io::{self, Write};
use std::sync::{atomic::AtomicBool, mpsc, Arc};
use std::thread;

pub fn run_tui<F>(
    title: &str,
    initial_total: usize,
    process_item: F,
    running_token: Arc<AtomicBool>,
) -> Result<()>
where
    F: FnOnce(mpsc::Sender<String>, Arc<AtomicBool>) -> Result<()> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<String>();

    let running_token_for_thread = running_token.clone();
    let processing_thread = thread::spawn(move || process_item(tx, running_token_for_thread));

    println!("🔄 {}", title);
    io::stdout().flush()?;

    let mut current_item_count = 0;
    let mut total_items = initial_total;

    while let Ok(message) = rx.recv() {
        // Handle special control messages
        if message.starts_with("TOTAL_FILES:") {
            if let Some(count_str) = message.split(':').nth(1) {
                if let Ok(count) = count_str.parse::<usize>() {
                    total_items = count;
                }
            }
            continue;
        }

        // Handle completion messages
        if message.starts_with("COMPLETED:") {
            current_item_count += 1;
            let task_description = message.strip_prefix("COMPLETED: ").unwrap_or(&message);

            // Simple progress display
            let progress = if total_items > 0 {
                (current_item_count as f64 / total_items as f64) * 100.0
            } else {
                100.0
            };

            print!("\r🔄 {}: {:.1}% ({}/{}) - {}",
                   title, progress, current_item_count, total_items, task_description);
        } else {
            // For other messages, just print them
            println!("\r🔄 {}: {}", title, message);
        }

        io::stdout().flush()?;
    }

    processing_thread.join().unwrap()?;

    // Final completion message
    println!("\r✅ {} completed! ({}/{})", title, current_item_count, total_items);

    Ok(())
}
