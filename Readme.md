# Hail - A Rust based scripting language
Hail is primarily inspired by Rhai (Hai, Hail, get it? Self insert? Whatever.) as well as Rust, aiming to make something comfortable for modders while being familar and easy to use for developers.

## Syntax
Simple Rust code can be copy-pasted straight into Hail, working exactly the same. Features such as lifetimes will not be found here though, making it more friendly to a "casual scripter".

```rust
// File: scripts/my_helper.hail
fn multiply(input: i64, factor: i64) -> i64 {
	// I know this is stupid, it's an example
	if factor == 0 { 0 } else { input * factor }
}
```

```rust
// File: scripts/my_example.hail
import "scripts/my_helper" as my_helper;

// Output:
// Result: 0
// Result: 5
for i in 0..2 {
	print("Result: " + my_helper::multiply(5, i));
}

// Output:
// Result: 20
// Result: 25
for i in [4, 5] {
	print("Result: " + my_helper::multiply(5, i));
}
```

## Usage
Hail comes with a built in standard library (hail_std) to work with, as well as a helper macro (hail_macro) to let you define your own interactions (everything is a function call, including operator overloading).

Voxel Eras' items (simplified) are defined as such:
```rust
pub fn generate_module() -> Result<hail::Module, hail::ModuleError> {
    let mut module = hail::Module::default();

    module.register_type_named::<ItemBuilder>("ItemBuilder")?;
    hail_register_item_builder(&mut module)?;
    hail_register_get_identifier(&mut module)?;
    hail_register_set_identifier(&mut module)?;
    hail_register_display_name(&mut module)?;

    Ok(module)
}

// "Free" is a function with no callee: ``ItemBuilder("StoneBrick:VE", "Stone Brick")``
#[hail(free, "ItemBuilder")]
pub fn item_builder(
    identifier: String,
    display_name: String,
) -> ItemBuilder {
    ItemBuilder {
        identifier,
        display_name,
		burn_ticks: None
    }
}

// "Property" returns a value of an object: ``builder.identifier``
#[hail(property, "identifier")]
pub fn get_identifier(builder: &ItemBuilder) -> String {
    builder.identifier.clone()
}

// "Setter" allows changing of a value of an object: ``builder.identifier = "Rock:VE";``
#[hail(setter, "identifier")]
pub fn set_identifier(builder: &mut ItemBuilder, identifier: String) {
    builder.identifier = identifier;
}

// "Method" operates with / on an object: ``builder.log();``
#[hail(method, "log")]
pub fn display_name(builder: &ItemBuilder) {
    println!("Logging ItemBuilder: '{builder:?}'");
}

// Optionally it can mutate the callee: ``builder.burnify(20);``
// Note: Chaining mutation calls can result in values not being stored as expected:
// ```hail
// let mut my_builder = ItemBuilder("A:VE", "A");
// let after = my_builder.burnify(5).burnify(10); // <- This results in my_builder having 5 burn_ticks, not 15.
// ```
// You can get around this in various ways, such as having builder functions return the modified builder.
#[hail(method, "burnify")]
pub fn burnify(builder: &mut ItemBuilder, burn_ticks: i64) {
    builder.burn_ticks = Some(burn_ticks);
}
```

## Performance
Early benchmarks seem to indicate a large increase in performance (take with a grain of salt):
- Hail: 0.677 seconds
- Rhai: 2.141 seconds

Which executes the following code:
```rust
const TARGET = 28;
const REPEAT = 5;
const ANSWER = 317811;

fn fib(n: i64) -> i64 {
	if n <= 1 {
		n
	} else {
		fib(n - 1) + fib(n - 2)
	}
}

for (let mut i = 0; i < REPEAT; i += 1;) {
	print(fib(TARGET));
}
```

## State
Hail is currently heavily a work in progress, but will be used by Voxel Eras (available on Steam) to replace Rhai in v1.5.0.

Here's a quick matrix of features I will / won't / are done.

### Done
- Typed: Hail is a typed language with compile time errors.
- Stack Machine: Unlike Rhai's AST Walker, Hail uses a much faster (but still inefficent) stack machine.
- Iterators, Ranges, For In Loops: Yep, yep, and yep! They're simple but functional and extendable.

### Will Do
- LSP, a typed language with a known API should have proper inlayed errors and hints
- Performance improvements, the stack machine does a lot of cloning of the stack / variables, it's still faster than Rhai.

### Won't Do
- Closures, currently I don't want to spend the time on these but I can see it as a *maybe* in the future.