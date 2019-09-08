use futures::future::Future;
use futures::stream::Stream;
use tail_rust::Tail;
fn main() {
    let path_str = "./data";
    tokio::run(
        Tail::new(path_str)
            .unwrap()
            .for_each(|line| {
                println!("tail: {}", line);
                Ok(())
            })
            .map_err(|e| ()),
    );
}
