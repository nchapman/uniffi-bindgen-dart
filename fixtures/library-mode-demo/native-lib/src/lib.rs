use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

// ── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error, uniffi::Error, Clone)]
pub enum ArithError {
    #[error("DivisionByZero")]
    DivisionByZero,
}

// ── Record ─────────────────────────────────────────────────────────────────

#[derive(uniffi::Record, Clone, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

// ── Enums ──────────────────────────────────────────────────────────────────

#[derive(uniffi::Enum, Clone, Debug)]
pub enum Shape {
    Circle { radius: f64 },
    Rect { w: f64, h: f64 },
}

// ── Custom type ────────────────────────────────────────────────────────────

pub struct Label(pub String);
uniffi::custom_newtype!(Label, String);

// ── Object ─────────────────────────────────────────────────────────────────

#[derive(uniffi::Object)]
pub struct Counter {
    value: AtomicI32,
}

#[uniffi::export]
impl Counter {
    #[uniffi::constructor]
    fn new(initial: i32) -> Arc<Self> {
        Arc::new(Self {
            value: AtomicI32::new(initial),
        })
    }

    fn get(&self) -> i32 {
        self.value.load(Ordering::SeqCst)
    }

    fn increment(&self) {
        self.value.fetch_add(1, Ordering::SeqCst);
    }

    #[uniffi::method(async_runtime = "tokio")]
    async fn async_get(&self) -> i32 {
        self.value.load(Ordering::SeqCst)
    }
}

// ── Top-level functions ────────────────────────────────────────────────────

#[uniffi::export]
fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

#[uniffi::export(async_runtime = "tokio")]
async fn greet_async(name: String) -> String {
    format!("Async hello, {name}!")
}

#[uniffi::export]
fn divide(a: u32, b: u32) -> Result<u32, ArithError> {
    if b == 0 {
        return Err(ArithError::DivisionByZero);
    }
    Ok(a / b)
}

#[uniffi::export]
fn echo_strings(v: Vec<String>) -> Vec<String> {
    v
}

#[uniffi::export]
fn echo_map(m: HashMap<String, i32>) -> HashMap<String, i32> {
    m
}

#[uniffi::export]
fn maybe_greet(name: Option<String>) -> Option<String> {
    name.map(|n| format!("Hello, {n}!"))
}

#[uniffi::export]
fn make_point(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[uniffi::export]
fn describe_shape(shape: Shape) -> String {
    match shape {
        Shape::Circle { radius } => format!("circle(r={radius})"),
        Shape::Rect { w, h } => format!("rect({w}x{h})"),
    }
}

#[uniffi::export]
fn echo_label(label: Label) -> Label {
    label
}

uniffi::setup_scaffolding!("library_mode_demo");
