fn hygiene_test() -> i32 {
    let x = 1;

    macro_rules! first_x {
        () => {
            x
        };
    }

    let x = 2;

    x + first_x!()
}

fn main() {
    println!("Hello, world!");
    let x = hygiene_test();
    println!("x is {x}");
}
