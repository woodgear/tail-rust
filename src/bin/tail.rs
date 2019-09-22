#![deny(warnings)]
#![deny(clippy::all)]

use futures::{future::Future, stream::Stream};
use tokio_threadpool::ThreadPool;

use tail_rust::Tail;

fn tail(path: String) {
    let thread_pool = ThreadPool::new();

    thread_pool.spawn(futures::lazy(move || {
        for i in Tail::new(&path).unwrap().wait() {
            println!("line {}", i.unwrap());
        }
        Ok(())
    }));

    thread_pool.shutdown().wait().unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args_slice: Vec<&str> = args.iter().map(std::string::String::as_str).collect();
    match args_slice[1..] {
        ["-f", path] => {
            println!("tail -f {}", path);
            tail(path.to_string());
        }
        _ => {
            println!("please use like tail -f xx but found {:?}", args_slice);
        }
    }
}
