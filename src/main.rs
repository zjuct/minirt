use minirt::{block_on, spawn};
use std::{time::Duration, borrow::BorrowMut};
use async_channel::{self, Sender, Receiver};
use std::future::Future;
use async_std;

use crate::timer_future::TimerFuture;

mod timer_future;

async fn _demo1() {
    println!("Hello");
}

async fn _demo2() {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::new(10, 0));
        tx.send_blocking("world")
    });
    println!("hello");
    let s = rx.recv().await.unwrap();
    println!("{}", s);
}

async fn _demo3() {
    spawn(_demo4());
    async_std::task::sleep(Duration::new(5, 0)).await;
    println!("Hello World!");
}

async fn _demo4() {
    println!("Hello World2!");
}


fn main() {
//    println!("hello");
//    block_on(
//        TimerFuture::new(Duration::new(5, 0))
//    );
//    println!("world");
    block_on(_demo3());
}
