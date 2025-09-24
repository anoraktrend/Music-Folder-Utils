use anyhow::Result;
use std::sync::{mpsc, Arc, atomic::AtomicBool};
use std::thread;
use std::io::{self, Write};

pub fn run_tui<F>(title: &str, initial_total: usize, process_item: F, running_token: Arc<AtomicBool>) -> Result<()> 
where
    F: FnOnce(mpsc::Sender<String>, Arc<AtomicBool>) -> Result<()> + Send + 'static, 
{
    let (tx, rx) = mpsc::channel::<String>();

    let running_token_for_thread = running_token.clone();
    let processing_thread = thread::spawn(move || {
        process_item(tx, running_token_for_thread)
    });

    let mut current_item_count = 0;
    let mut total_items = initial_total;
    // Print initial display (title + 2 empty lines for progress display)
    // Title line, then two empty lines (one for the progress bar, one for the current item)
    print!("Starting: {}\n\n\n", title);
    io::stdout().flush()?;

    while let Ok(name) = rx.recv() {
        // Check for special control messages
        if name.starts_with("TOTAL_FILES:") {
            if let Some(count_str) = name.split(':').nth(1) {
                if let Ok(count) = count_str.parse::<usize>() {
                    total_items = count;
                    continue; // Skip normal processing for this message
                }
            }
        }

        // Check for completed task messages
        let current_item_name = if name.starts_with("COMPLETED:") {
            current_item_count += 1;
            let task_description = name.strip_prefix("COMPLETED: ").unwrap_or(&name);
            format!("✓ {}", task_description)
        } else {
            // For non-COMPLETED messages, just display them without counting as progress
            name
        };

        // Drain any queued messages to keep the UI up-to-date when producers send
        // bursts of updates. For each drained message increment the count and
        // update the displayed name to the most recent.
        for queued in rx.try_iter() {
            if queued.starts_with("TOTAL_FILES:") {
                if let Some(count_str) = queued.split(':').nth(1) {
                    if let Ok(count) = count_str.parse::<usize>() {
                        total_items = count;
                        continue;
                    }
                }
            }
            let _current_item_name = if queued.starts_with("COMPLETED:") {
                current_item_count += 1;
                let task_description = queued.strip_prefix("COMPLETED: ").unwrap_or(&queued);
                format!("✓ {}", task_description)
            } else {
                // For non-COMPLETED messages, just update the display without counting as progress
                queued
            };
        }

        // Avoid divide-by-zero when there are no items
        let progress = if total_items == 0 {
            100.0
        } else {
            (current_item_count as f64 / total_items as f64) * 100.0
        };

        // Determine terminal width and compute bar length to fill available space.
        // Reserve space for the percentage and counts text and a small padding.
        let cols = crossterm::terminal::size().map(|(c, _r)| c as usize).unwrap_or(80);
        let meta = format!(" {:.2}% - {}/{}", progress, current_item_count, total_items);
        // Reserve for brackets + one space padding
        let reserved = meta.len().saturating_add(3);
        let bar_length = if cols > reserved { cols - reserved } else { 10 };
        let mut filled_length = ((progress / 100.0) * bar_length as f64).round() as usize;
        if filled_length > bar_length { filled_length = bar_length; }
        let bar = "#".repeat(filled_length) + &"-".repeat(bar_length - filled_length);

        // Clear screen and redraw everything for consistent display
        print!("\x1B[2J\x1B[H"); // Clear screen and move cursor to top-left
        print!("Starting: {}\n", title);

        // Print progress bar
        print!("[{}] {:.2}% - {}/{}\n", bar, progress, current_item_count, total_items);
        // Print current item
        print!("{}\n", current_item_name);
        io::stdout().flush()?;
    }

    processing_thread.join().unwrap()?;

    // Keep the final progress bar and item visible, just add the finished message
    // Move cursor down to add the completion message without clearing
    print!("\n\x1B[1;32m✓ Finished: {}\x1B[0m\n", title);
    io::stdout().flush()?;
    Ok(())
}
