use high_order_locks::{AcquiredLock, Client, Forkable, Lock};

#[tokio::main]
async fn main() {
    let lock = Lock::rev(42).await;

    let lock = lock
        .fork(|lock: Lock<_, Client>| async move {
            let (lock, value) = lock.acquire().await;
            let lock = lock.release(value * 2).await;
            let (lock, value) = lock.acquire().await;

            let lock = lock
                .fork(move |lock: AcquiredLock<_, Client>| async move {
                    let lock = lock.release(value + 17).await;
                    lock.drop().await;
                })
                .await;

            lock.drop().await;
        })
        .await;

    let (lock, v) = lock.acquire().await;

    let lock = lock
        .fork(move |lock: AcquiredLock<_, Client>| async move {
            let lock = lock.release(v + 42).await;
            lock.drop().await;
        })
        .await;

    let value = lock.wait().await;

    println!("value = {value:?}");
}
