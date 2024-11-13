use std::{fmt::Debug, future::Future, marker::PhantomData, sync::Arc};

use tokio::{
    spawn,
    sync::{Mutex, Notify},
};

/// a = 1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner;
/// a = 0
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client;

/// b = 1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Acquired;
/// b = 0
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAcquired;

#[derive(Debug)]
pub struct Lock<T, A, B> {
    inner: Arc<Mutex<Option<T>>>,
    phantom_data: PhantomData<(A, B)>,
    notify: Arc<Notify>,
    clients: Arc<Mutex<usize>>,
}

impl<T, A> Lock<T, A, NotAcquired>
where
    T: Clone,
{
    pub async fn acquire(self) -> (Lock<T, A, Acquired>, T) {
        let Lock {
            inner,
            notify,
            clients,
            ..
        } = self;

        let value = { inner.lock().await.take().unwrap() };

        (
            Lock {
                inner,
                phantom_data: PhantomData,
                notify,
                clients,
            },
            value,
        )
    }

    pub async fn get(self) -> (Lock<T, A, NotAcquired>, T) {
        let (lock, value) = self.acquire().await;
        (lock.release(value.clone()).await, value)
    }

    pub async fn set(self, value: T) -> Lock<T, A, NotAcquired> {
        let (lock, _) = self.acquire().await;
        lock.release(value).await
    }

    pub async fn exchange(self, value: T) -> (Lock<T, A, NotAcquired>, T) {
        let (lock, old) = self.acquire().await;
        (lock.release(value).await, old)
    }

    pub async fn modify(self, modifier: &dyn Fn(T) -> T) -> Lock<T, A, NotAcquired> {
        let (lock, value) = self.acquire().await;

        lock.release(modifier(value)).await
    }
}

impl<T, A> Lock<T, A, Acquired> {
    pub async fn release(self, value: T) -> Lock<T, A, NotAcquired> {
        let Lock {
            inner,
            notify,
            clients,
            ..
        } = self;
        {
            inner.lock().await.replace(value);
        }

        Lock {
            inner,
            phantom_data: PhantomData,
            notify,
            clients,
        }
    }
}

impl<T> Lock<T, Owner, Acquired> {
    pub async fn new() -> Lock<T, Owner, Acquired> {
        let notify = Notify::new();

        Lock {
            inner: Arc::new(Mutex::new(None)),
            phantom_data: PhantomData,
            notify: Arc::new(notify),
            clients: Arc::new(Mutex::new(0)),
        }
    }
}

impl<T> Lock<T, Owner, NotAcquired> {
    pub async fn wait(self) -> T {
        let Lock {
            inner,
            notify,
            clients,
            ..
        } = self;
        loop {
            let clients = *clients.lock().await;

            if clients == 0 {
                return inner.lock().await.take().expect("Something went wrong");
            }

            notify.notified().await;
        }
    }
}

impl<T> Lock<T, Client, NotAcquired> {
    pub async fn drop(self) {
        let Lock {
            notify, clients, ..
        } = self;

        let mut clients = clients.lock().await;
        *clients -= 1;

        if *clients == 0 {
            notify.notify_one();
        }
    }
}

impl<T, A, B> Lock<T, A, B>
where
    T: Send + 'static,
{
    pub async fn fork<Func, Ret>(self, closure: Func) -> Lock<T, A, B>
    where
        Func: (Fn(Lock<T, Client, NotAcquired>) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static,
    {
        let Lock {
            inner,
            notify,
            clients,
            ..
        } = self;

        let new_lock = Lock {
            inner: inner.clone(),
            notify: notify.clone(),
            clients: clients.clone(),
            phantom_data: PhantomData,
        };

        {
            *clients.lock().await += 1;
        }

        spawn(async move { closure(new_lock).await });

        Lock {
            inner,
            phantom_data: PhantomData,
            notify,
            clients,
        }
    }
}

impl<T> Lock<T, Owner, Acquired> {
    pub async fn rev(value: T) -> Lock<T, Owner, NotAcquired> {
        let lock = Lock::new().await.release(value).await;
        lock.notify.notify_one();
        lock
    }
}
