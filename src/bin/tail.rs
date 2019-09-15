use futures::future::Future;
use futures::stream::Stream;
use tail_rust::Tail;

fn tail(path: String) {
    tokio::run(
        Tail::new(&path)
            .unwrap()
            .for_each(|line| {
                println!("tail: {}", line);
                Ok(())
            })
            .map_err(|_| ()),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let args_slice: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
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
