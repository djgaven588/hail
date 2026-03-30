use std::fs;

use hail::run;

fn main() {
    let mut args = std::env::args();
    if args.len() < 2 {
        println!("Usage: hail [script]");
        return;
    } else if args.len() > 2 {
        println!("Usage: hail [script], unexpected args.");
        return;
    }
    let _run_path = args.next().unwrap();
    let script = args.next().unwrap();
    println!("Running: {script}");
    let source = fs::read_to_string(script).expect("Script should be at location.");
    run(source);
}
