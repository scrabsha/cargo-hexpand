<div class="title-block" style="text-align: center;" align="center">

# cargo-hexpand

`cargo-expand`, but with Hygiene*.

<sub>\*Still very WIP.</sub>


</div>

## The problem

[`cargo-expand`] works well, but it does not respect hygiene when expanding the macros. As a result, the expanded code may behave diffentently. Here's an example:

```rust
fn f() -> i32 {
    let x = 1;

    macro_rules! first_x {
        () => { x }
    }

    let x = 2;

    x + first_x!()
}
```

In this example, the `x` coming from the expansion has _call-site_ hygiene. At least, it should resolve to the `x` that is defined in the first statement of the function. The `f` function should return 3.

## How `cargo-hexpand` fixes this

`cargo-hexpand` uses the rustc internal API in order to detect shadowing that alter the program behavior and rename rename the variables so that no semantic change occurs. This leads to the following expansion:

```rust
use std::prelude::rust_2021::*;
extern crate std;
fn hygiene_test() -> i32 {
    let x = 1;

    macro_rules! first_x {
        () => {
            x
        };
    }

    let x_0 = 2;

    x_0 + x
}
```

The second `x` was renamed to `x_0`, so that no shadowing occurs. `f` returns 3 as well. Great!

## Installation instructions

TODO

(nightly, rustc-dev, ...)

[`cargo-expand`]: https://github.com/dtolnay/cargo-expand
