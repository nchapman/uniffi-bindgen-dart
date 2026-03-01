fn main() {
    if let Err(err) = ubdg_bindgen::run(std::env::args_os()) {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
