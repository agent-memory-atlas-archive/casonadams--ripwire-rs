fn main() {
    if let Err(err) = ripwire_rs::run_cli() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
