use estado::prelude::*;

fn not_a_handler() {
    let _ = goto!(Playing);
}

fn also_not_a_handler() {
    let _ = estado::stay!();
}

fn not_a_type() -> Option<edges![A | B]> {
    None
}

fn main() {}
