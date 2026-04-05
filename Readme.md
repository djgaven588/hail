# Hail - A Rust based scripting language
Hail is primarily inspired by Rhai (Hai, Hail, get it? Self insert? Whatever.) as well as Rust, aiming to make something comfortable for modders while being familar and easy to use for developers.

It has 2 executors implemented:
- An AST Walker, akin to Rhai's executor (slower)
- A Stack Machine (faster)

Early benchmarks seem to indicate a doubling of performance for the AST Walker, and more for the stack machine.
Here's an example:
- Hail Walker: 0.855 seconds
- Hail Machine: 0.677 seconds
- Rhai Walker: 2.141 seconds

Which executes the following code:
```rust
const TARGET = 28;
const REPEAT = 5;
const ANSWER = 317811;

fn fib(n) {
	if n <= 1 {
		n
	} else {
		fib(n - 1) + fib(n - 2)
	}
}

for (let mut i = 0; i < REPEAT; i += 1;) {
	print fib(TARGET);
}
```

It is currently heavily a work in progress, but will be used by Voxel Eras (available on Steam) to replace Rhai.
