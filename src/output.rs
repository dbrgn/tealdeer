//! Functions for printing pages to the terminal

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result};

use crate::{
    cache::PageLookupResult,
    config::{Config, StyleConfig},
    formatter::{highlight_lines, PageSnippet},
    line_iterator::LineIterator,
};

// Set up display pager
#[cfg(not(target_os = "windows"))]
fn configure_pager(_: bool) {
    pager::Pager::with_default_pager("less -R").setup();
}

#[cfg(target_os = "windows")]
fn configure_pager(enable_styles: bool) {
    use crate::utils::print_warning;
    print_warning(enable_styles, "--pager flag not available on Windows!");
}

/// Print page by path
pub fn print_page(
    lookup_result: &PageLookupResult,
    enable_markdown: bool,
    enable_styles: bool,
    use_pager: bool,
    config: &Config,
) -> Result<()> {
    // Create reader from file(s)
    let reader = lookup_result.reader()?;

    // Configure pager if applicable
    if use_pager || config.display.use_pager {
        configure_pager(enable_styles);
    }

    // Lock stdout only once, this improves performance considerably
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    if enable_markdown {
        // Print the raw markdown of the file.
        for line in reader.lines() {
            let line = line.context("Error while reading from a page")?;
            writeln!(handle, "{}", line).context("Could not write to stdout")?;
        }
    } else {
        // Closure that processes a page snippet and writes it to stdout
        let mut process_snippet = |snip: PageSnippet<'_>| {
            if snip.is_empty() {
                Ok(())
            } else {
                print_snippet(&mut handle, snip, &config.style).context("Failed to print snippet")
            }
        };

        // Print highlighted lines
        highlight_lines(
            LineIterator::new(reader),
            &mut process_snippet,
            !config.display.compact,
        )
        .context("Could not write to stdout")?;
    };

    // We're done outputting data, flush stdout now!
    handle.flush().context("Could not flush stdout")?;

    Ok(())
}

fn print_snippet(
    writer: &mut impl Write,
    snip: PageSnippet<'_>,
    style: &StyleConfig,
) -> io::Result<()> {
    use PageSnippet::*;

    match snip {
        CommandName(s) => write!(writer, "{}", style.command_name.paint(s)),
        Variable(s) => write!(writer, "{}", style.example_variable.paint(s)),
        NormalCode(s) => write!(writer, "{}", style.example_code.paint(s)),
        Description(s) => writeln!(writer, "  {}", style.description.paint(s)),
        Text(s) => writeln!(writer, "  {}", style.example_text.paint(s)),
        Linebreak => writeln!(writer),
    }
}
