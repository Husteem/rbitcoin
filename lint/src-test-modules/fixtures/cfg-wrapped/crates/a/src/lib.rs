#[cfg(all(
    test,
    target_os = "linux",
    feature = "not-a-test-name"
))]
mod wrapped {
    #[test]
    fn inline() {}
}
