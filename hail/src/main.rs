use hail::run;

fn main() {
    let mut args = std::env::args();
    if args.len() < 2 {
        println!("Usage: hail [script]");
        return;
    } else if args.len() > 3 {
        println!("Usage: hail [script], unexpected args.");
        return;
    }
    let _run_path = args.next().unwrap();
    let next = args.next().unwrap();
    let is_bench = next == "--bench";
    let script = if is_bench { args.next().unwrap() } else { next };
    run(script, is_bench);
}
