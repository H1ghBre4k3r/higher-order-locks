use high_order_locks::{Lock, NotAcquired};

#[tokio::main]
async fn main() {
    let account1 = Lock::rev(500).await;
    let account2 = Lock::rev(100).await;

    let (account1, account2) = transaction(account1, account2, 200).await;

    let b1 = account1.wait().await;
    let b2 = account2.wait().await;

    println!("{b1} {b2}");
}

async fn transaction<A>(
    l1: Lock<i32, A, NotAcquired>,
    l2: Lock<i32, A, NotAcquired>,
    amount: i32,
) -> (Lock<i32, A, NotAcquired>, Lock<i32, A, NotAcquired>) {
    let (l1, b1) = l1.acquire().await;
    let (l2, b2) = l2.acquire().await;

    if b1 >= amount {
        (l1.release(b1 - amount).await, l2.release(b2 + amount).await)
    } else {
        (l1.release(b1).await, l2.release(b2).await)
    }
}
