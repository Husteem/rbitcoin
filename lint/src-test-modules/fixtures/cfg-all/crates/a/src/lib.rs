#[cfg(all(test, target_os = "linux"))]
mod pinned {
    #[test]
    fn inline() {}
}
