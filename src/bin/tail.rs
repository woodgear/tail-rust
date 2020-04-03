#![deny(warnings)]
#![deny(clippy::all)]

use futures::{stream::{StreamExt}};
use tail_rust::Tail;

async fn tail(path:&str) {
    let mut tail = Tail::new(path);
    while  let Some(Ok(line)) = tail.next().await {
        println!("{}",line);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args_slice: Vec<&str> = args.iter().map(std::string::String::as_str).collect();
    match args_slice[1..] {
        ["-f", path] => {
            println!("tail -f {}", path);
            tokio::runtime::Runtime::new().unwrap().block_on(tail(path));
        }
        _ => {
            println!("please use like tail -f xx but found {:?}", args_slice);
        }
    }
}
