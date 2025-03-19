// Goals for canister shape
// 1. easy to unit test (avoid things that can only be tested in WASM32 statemachine)
// 2. Simple structures
// 3. Good API boundaries (nothing on the inside gets out)
// 4. Structure makes boundaries clear and easy to enforce
// 5. Simple Organization

pub mod canister;
pub mod storage;

#[cfg(test)]
mod tests {
    #[test]
    fn dummy_test_to_confirm_tests_are_being_run() {
        assert_eq!(2 + 2, 4);
    }
}
