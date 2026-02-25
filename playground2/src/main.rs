use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread::sleep;
use std::time::Duration;

fn main() {
    // Give time to switch to a text editor
    println!("Starting in 3 seconds, switch to a text editor...");
    sleep(Duration::from_secs(3));

    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    // Test 1: plain text, no newlines
    println!("Test 1: plain text");
    let r = enigo.text("hello world");
    println!("  result: {r:?}");
    sleep(Duration::from_millis(500));

    // Test 2: single newline
    println!("Test 2: single newline");
    let r = enigo.text("line1\nline2");
    println!("  result: {r:?}");
    sleep(Duration::from_millis(500));

    // Test 3: multiple newlines (the bug case)
    println!("Test 3: multiple newlines");
    let r = enigo.text("i= 0\ni= 1\ni= 2\ni= 3\ni= 4");
    println!("  result: {r:?}");
    sleep(Duration::from_millis(500));

    println!("Done.");
}
