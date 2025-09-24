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

    let mut current_item_count = 0;
    let mut total_items = initial_total;

    // Get terminal dimensions
    let (cols, rows) = crossterm::terminal::size()
        .map(|(c, r)| (c as usize, r as usize))
        .unwrap_or((80, 24));

    // Print initial display (title at top)
    println!("Starting: {}", title);
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
        let meta = format!(" {:.2}% - {}/{}", progress, current_item_count, total_items);
        // Reserve for brackets + one space padding
        let reserved = meta.len().saturating_add(3);
        let bar_length = if cols > reserved { cols - reserved } else { 10 };
        let mut filled_length = ((progress / 100.0) * bar_length as f64).round() as usize;
        if filled_length > bar_length {
            filled_length = bar_length;
        }
        let bar = "#".repeat(filled_length) + &"-".repeat(bar_length - filled_length);

        // Clear screen and position cursor at bottom for progress bar
        print!("\x1B[2J"); // Clear screen
        print!("\x1B[{}A", rows - 2); // Move cursor up to near bottom (leave space for title and progress)

        // Print title at top
        println!("Starting: {}", title);

        // Position cursor at bottom for progress bar
        print!("\x1B[{}B", rows - 3); // Move down to bottom area

        // Print progress bar at bottom
        print!("[{}]{} \x1B[1A", bar, meta); // Print progress bar and move up one line

        // Print current item above progress bar
        print!("\x1B[1A{}\x1B[1B", current_item_name); // Move up, print item, move back down

        io::stdout().flush()?;
    }

    processing_thread.join().unwrap()?;

    // Final display - keep progress bar at bottom with completion message
    print!("\x1B[2J"); // Clear screen
    print!("\x1B[{}A", rows - 3); // Move cursor up to near bottom
    println!("Starting: {}", title);
    print!("\x1B[{}B", rows - 4); // Move down to bottom area

    // Print final progress bar
    let progress = 100.0;
    let meta = format!(" {:.2}% - {}/{}", progress, total_items, total_items);
    let bar_length = if cols > meta.len().saturating_add(3) {
        cols - meta.len().saturating_add(3)
    } else {
        10
    };
    let bar = "#".repeat(bar_length);
    print!("[{}]{} \x1B[1A", bar, meta);

    // Print completion message above progress bar
    print!("\x1B[1A\x1B[1;32m✓ Finished: {}\x1B[0m\x1B[1B", title);

    io::stdout().flush()?;
    Ok(())
}
