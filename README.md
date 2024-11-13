# Higher-Order Leak and Deadlock Free Locks

Implementation of the paper "Higher-Order Leak and Deadlock Free Locks" in Rust 🦀

## Reference

The code in this repo is based on the named paper. If you want to cite it:

```bibtex
@article{10.1145/3571229,
author = {Jacobs, Jules and Balzer, Stephanie},
title = {Higher-Order Leak and Deadlock Free Locks},
year = {2023},
issue_date = {January 2023},
publisher = {Association for Computing Machinery},
address = {New York, NY, USA},
volume = {7},
number = {POPL},
url = {https://doi.org/10.1145/3571229},
doi = {10.1145/3571229},
abstract = {Reasoning about concurrent programs is challenging, especially if data is shared among threads. Program correctness can be violated by the presence of data races—whose prevention has been a topic of concern both in research and in practice. The Rust programming language is a prime example, putting the slogan fearless concurrency in practice by not only employing an ownership-based type system for memory management, but also using its type system to enforce mutual exclusion on shared data. Locking, unfortunately, not only comes at the price of deadlocks but shared access to data may also cause memory leaks. This paper develops a theory of deadlock and leak freedom for higher-order locks in a shared memory concurrent setting. Higher-order locks allow sharing not only of basic values but also of other locks and channels, and are themselves first-class citizens. The theory is based on the notion of a sharing topology, administrating who is permitted to access shared data at what point in the program. The paper first develops higher-order locks for acyclic sharing topologies, instantiated in a λ-calculus with higher-order locks and message-passing concurrency. The paper then extends the calculus to support circular dependencies with dynamic lock orders, which we illustrate with a dynamic version of Dijkstra’s dining philosophers problem. Well-typed programs in the resulting calculi are shown to be free of deadlocks and memory leaks, with proofs mechanized in the Coq proof assistant.},
journal = {Proc. ACM Program. Lang.},
month = jan,
articleno = {36},
numpages = {31},
keywords = {Memory Leak, Higher-Order Lock, Deadlock, Concurrency}
}
```
