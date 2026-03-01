uniffi::setup_scaffolding!();

#[uniffi::export]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

#[derive(uniffi::Record)]
struct TodoEntry {
    title: String,
    done: bool,
}
