const TARGET = 28;
const REPEAT = 5;
const ANSWER = 317_811;

fn fib(n) {
	if n <= 1 {
		n
	} else {
		fib(n - 1) + fib(n - 2)
	}
}

for i in 0..REPEAT {
	print(fib(TARGET));
}