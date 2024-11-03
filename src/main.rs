use std;
use std::io::stderr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = fetter::run_cli(std::env::args_os()) {
        let mut stderr = stderr();
        fetter::write_color(&mut stderr, "#666666", "fetter ");
        fetter::write_color(&mut stderr, "#cc0000", "Error: ");
        eprintln!("{}", e);
        std::process::exit(1);
    }
    Ok(())
}
