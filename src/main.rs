use high_order_locks::{Client, Forkable, Lock, NotAcquired, Owner};

#[tokio::main]
async fn main() {
    let lock = Lock::rev(42).await;

    let lock = lock
        .fork(|lock: Lock<_, Client, NotAcquired>| async move {
            let (lock, value) = lock.acquire().await;
            let lock = lock.release(value * 2).await;

            let lock = lock
                .fork(|lock: Lock<_, Client, NotAcquired>| async move {
                    let (lock, value) = lock.acquire().await;
                    let lock = lock.release(value + 17).await;
                    lock.drop().await;
                })
                .await;

            lock.drop().await;
        })
        .await;

    let value = lock.wait().await;

    println!("value = {value:?}");
}
