use std::{fmt::Debug, future::Future, marker::PhantomData, sync::Arc};

use tokio::{
    spawn,
    sync::{Mutex, Notify, OwnedMutexGuard},
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
pub struct Lock<T, A> {
    inner: Arc<Mutex<Option<T>>>,
    phantom_data: PhantomData<A>,
    notify: Arc<Notify>,
    clients: Arc<Mutex<usize>>,
}

#[derive(Debug)]
pub struct AcquiredLock<T, A> {
    inner: Arc<Mutex<Option<T>>>,
    data: OwnedMutexGuard<Option<T>>,
    phantom_data: PhantomData<A>,
    notify: Arc<Notify>,
    clients: Arc<Mutex<usize>>,
}

impl<T, A> Lock<T, A>
where
    T: Clone,
{
    #[must_use]
    pub async fn acquire(self) -> (AcquiredLock<T, A>, T) {
        let Lock {
            inner,
            notify,
            clients,
            ..
        } = self;

        let mut data = inner.clone().lock_owned().await;
        let value = data.take().unwrap();

        (
            AcquiredLock {
                inner,
                data,
                phantom_data: PhantomData,
                notify,
                clients,
            },
            value,
        )
    }

    #[must_use]
    pub async fn get(self) -> (Lock<T, A>, T) {
        let (lock, value) = self.acquire().await;
        (lock.release(value.clone()).await, value)
    }

    #[must_use]
    pub async fn set(self, value: T) -> Lock<T, A> {
        let (lock, _) = self.acquire().await;
        lock.release(value).await
    }

    #[must_use]
    pub async fn exchange(self, value: T) -> (Lock<T, A>, T) {
        let (lock, old) = self.acquire().await;
        (lock.release(value).await, old)
    }

    #[must_use]
    pub async fn modify(self, modifier: &dyn Fn(T) -> T) -> Lock<T, A> {
        let (lock, value) = self.acquire().await;

        lock.release(modifier(value)).await
    }
}

impl<T, A> AcquiredLock<T, A> {
    #[must_use]
    pub async fn release(self, value: T) -> Lock<T, A> {
        let AcquiredLock {
            inner,
            notify,
            clients,
            mut data,
            ..
        } = self;

        data.replace(value);

        Lock {
            inner,
            phantom_data: PhantomData,
            notify,
            clients,
        }
    }
}

impl<T> AcquiredLock<T, Owner> {
    #[must_use]
    pub async fn new() -> AcquiredLock<T, Owner> {
        let notify = Notify::new();

        let inner = Arc::new(Mutex::new(None));
        let data = inner.clone().lock_owned().await;

        AcquiredLock {
            inner,
            data,
            phantom_data: PhantomData,
            notify: Arc::new(notify),
            clients: Arc::new(Mutex::new(0)),
        }
    }
}

impl<T> Lock<T, Owner> {
    #[must_use]
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

    #[must_use]
    pub async fn rev(value: T) -> Lock<T, Owner> {
        let lock = AcquiredLock::new().await.release(value).await;
        lock.notify.notify_one();
        lock
    }
}

impl<T> Lock<T, Client> {
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

pub trait Forkable<T, PassedLock, ReturnedLock>
where
    T: Send + 'static,
{
    #[must_use]
    fn fork<Func, Ret>(self, closure: Func) -> impl Future<Output = ReturnedLock>
    where
        Func: (Fn(PassedLock) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static;
}

impl<T> Forkable<T, Lock<T, Client>, Lock<T, Owner>> for Lock<T, Owner>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> Lock<T, Owner>
    where
        Func: (Fn(Lock<T, Client>) -> Ret) + Sync + Send + 'static,
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

impl<T> Forkable<T, Lock<T, Owner>, Lock<T, Client>> for Lock<T, Owner>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> Lock<T, Client>
    where
        Func: (Fn(Lock<T, Owner>) -> Ret) + Sync + Send + 'static,
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

impl<T> Forkable<T, Lock<T, Client>, Lock<T, Client>> for Lock<T, Client>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> Lock<T, Client>
    where
        Func: (Fn(Lock<T, Client>) -> Ret) + Sync + Send + 'static,
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

impl<T> Forkable<T, AcquiredLock<T, Client>, Lock<T, Owner>> for AcquiredLock<T, Owner>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> Lock<T, Owner>
    where
        Func: (Fn(AcquiredLock<T, Client>) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static,
    {
        let AcquiredLock {
            inner,
            data,
            notify,
            clients,
            ..
        } = self;

        let new_lock = AcquiredLock {
            inner: inner.clone(),
            data,
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
            notify,
            clients,
            phantom_data: PhantomData,
        }
    }
}

impl<T> Forkable<T, Lock<T, Client>, AcquiredLock<T, Owner>> for AcquiredLock<T, Owner>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> AcquiredLock<T, Owner>
    where
        Func: (Fn(Lock<T, Client>) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static,
    {
        let AcquiredLock {
            inner,
            data,
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

        AcquiredLock {
            inner,
            data,
            notify,
            clients,
            phantom_data: PhantomData,
        }
    }
}

impl<T> Forkable<T, AcquiredLock<T, Owner>, Lock<T, Client>> for AcquiredLock<T, Owner>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> Lock<T, Client>
    where
        Func: (Fn(AcquiredLock<T, Owner>) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static,
    {
        let AcquiredLock {
            inner,
            data,
            notify,
            clients,
            ..
        } = self;

        let new_lock = AcquiredLock {
            inner: inner.clone(),
            data,
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
            notify,
            clients,
            phantom_data: PhantomData,
        }
    }
}

impl<T> Forkable<T, Lock<T, Owner>, AcquiredLock<T, Client>> for AcquiredLock<T, Owner>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> AcquiredLock<T, Client>
    where
        Func: (Fn(Lock<T, Owner>) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static,
    {
        let AcquiredLock {
            inner,
            data,
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

        AcquiredLock {
            inner,
            data,
            notify,
            clients,
            phantom_data: PhantomData,
        }
    }
}

impl<T> Forkable<T, AcquiredLock<T, Client>, Lock<T, Client>> for AcquiredLock<T, Client>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> Lock<T, Client>
    where
        Func: (Fn(AcquiredLock<T, Client>) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static,
    {
        let AcquiredLock {
            inner,
            data,
            notify,
            clients,
            ..
        } = self;

        let new_lock = AcquiredLock {
            inner: inner.clone(),
            data,
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
            notify,
            clients,
            phantom_data: PhantomData,
        }
    }
}

impl<T> Forkable<T, Lock<T, Client>, AcquiredLock<T, Client>> for AcquiredLock<T, Client>
where
    T: Send + 'static,
{
    async fn fork<Func, Ret>(self, closure: Func) -> AcquiredLock<T, Client>
    where
        Func: (Fn(Lock<T, Client>) -> Ret) + Sync + Send + 'static,
        Ret: Future<Output = ()> + Send + 'static,
    {
        let AcquiredLock {
            inner,
            data,
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

        AcquiredLock {
            inner,
            data,
            notify,
            clients,
            phantom_data: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::test;

    use crate::{AcquiredLock, Lock, Owner};

    #[test]
    async fn test_new() {
        let _ = AcquiredLock::<i32, Owner>::new().await;
    }

    #[test]
    async fn test_release() {
        let lock = AcquiredLock::<i32, Owner>::new().await;
        let _ = lock.release(42).await;
    }

    #[test]
    async fn test_acquire() {
        let lock = AcquiredLock::<i32, Owner>::new().await;
        let lock = lock.release(42).await;
        let (_, v) = lock.acquire().await;
        assert_eq!(v, 42);
    }

    #[test]
    async fn test_rev() {
        let lock = Lock::rev(42).await;
        let (_, v) = lock.acquire().await;
        assert_eq!(v, 42);
    }
}
