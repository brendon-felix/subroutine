fn main() {
    let action = simple_core::Action::new("test title");
    println!("{}", serde_json::to_string_pretty(&action).unwrap());
}
