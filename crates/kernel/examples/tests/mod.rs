test_case!(example_test);
fn example_test() {
    println!("Running test one");
}

// test_case!(async example_failure);
// async fn example_failure() -> Result<(), BoxError> {
//     println!("Running test two");
//     sync::spin_sleep(500000);
//     kassert!(false, "hello?")?;
//     Ok(())
// }
