use assert_matches::assert_matches;
use ic_crypto_tree_hash::{
    prune_witness,
    HashTreeBuilder,
    Witness,
    WitnessGenerator,
};
use ic_crypto_tree_hash_test_utils::{
    arbitrary::arbitrary_labeled_tree,
    hash_tree_builder_from_labeled_tree,
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn hash_tree_builder_from_labeled_tree_works_correctly(tree in arbitrary_labeled_tree()){
        let builder = hash_tree_builder_from_labeled_tree(&tree);
        // check that the witness is correct by pruning it completely
        let wg = builder
            .witness_generator()
            .expect("Failed to retrieve a witness constructor");
        let witness = wg
            .witness(&tree)
            .expect("Failed to build a witness for the whole tree");
        let witness = prune_witness(&witness, &tree).expect("failed to prune witness");
        assert_matches!(witness, Witness::Pruned { digest: _ });
    }
}
