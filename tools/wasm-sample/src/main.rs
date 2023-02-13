fn main() {
    println!("Printing args");
    for arg in std::env::args().skip(1) {
        println!("{arg}");
    }

    println!("Printing envs");
    for envs in std::env::vars() {
        println!("{envs:?}");
    }
}
