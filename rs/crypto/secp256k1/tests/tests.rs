use hex_literal::hex;
use ic_crypto_secp256k1::{DerivationPath, KeyDecodingError, PrivateKey, PublicKey};
use rand::Rng;
use rand_chacha::ChaCha20Rng;

fn test_rng_with_seed(seed: [u8; 32]) -> ChaCha20Rng {
    use rand::SeedableRng;
    ChaCha20Rng::from_seed(seed)
}

fn test_rng() -> ChaCha20Rng {
    let seed = rand::thread_rng().gen::<[u8; 32]>();
    // If a test ever fails, reproduce it using
    // let mut rng = test_rng_with_seed(hex!("SEED"));
    println!("RNG seed: {}", hex::encode(seed));
    test_rng_with_seed(seed)
}

#[test]
fn should_pass_wycheproof_ecdsa_secp256k1_verification_tests() -> Result<(), KeyDecodingError> {
    use wycheproof::ecdsa::*;

    let test_set =
        TestSet::load(TestName::EcdsaSecp256k1Sha256P1363).expect("Unable to load test set");

    for test_group in &test_set.test_groups {
        let pk = PublicKey::deserialize_sec1(&test_group.key.key)?;
        let pk_der = PublicKey::deserialize_der(&test_group.der)?;
        assert_eq!(pk, pk_der);

        for test in &test_group.tests {
            // The Wycheproof ECDSA tests do not normalize s so we must use
            // the verification method that accepts either valid s
            let accepted = pk.verify_ecdsa_signature_with_malleability(&test.msg, &test.sig);
            assert_eq!(accepted, test.result == wycheproof::TestResult::Valid);
        }
    }

    Ok(())
}

#[test]
fn test_sign_prehash_works_with_any_size_input() {
    let rng = &mut test_rng();

    let sk = PrivateKey::generate_using_rng(rng);
    let pk = sk.public_key();

    for i in 0..1024 {
        let buf = vec![0x42; i];
        let sig = sk.sign_digest_with_ecdsa(&buf);
        assert!(pk.verify_ecdsa_signature_prehashed(&buf, &sig));
    }
}

#[test]
fn should_use_rfc6979_nonces_for_ecdsa_signature_generation() {
    // Unfortunately RFC 6979 does not include tests for secp256k1. This
    // signature was instead generated by another implementation that both supports
    // secp256k1 ECDSA and uses RFC 6979 nonce generation.

    let sk = PrivateKey::deserialize_sec1(
        &hex::decode("8f44c8e5da21a3e2933fbf732519a604891b4731f19045f078e6ce57893c1f2a")
            .expect("Valid hex"),
    )
    .expect("Valid key");

    let message = b"abc";
    let expected_sig = "d8bdb0ddfc8ebb8be42649048e92edc8547d1587b2a8f721738a2ecc0733401c70e86d3042ebbb50dccfbfbdf6c0462c7be45bcd0208d33e34efec273a86eab9";

    let generated_sig = sk.sign_message_with_ecdsa(message);
    assert_eq!(hex::encode(generated_sig), expected_sig);

    // Now check the prehash variant:
    let message_hash: [u8; 32] = {
        use sha2::Digest;
        let mut sha256 = sha2::Sha256::new();
        sha256.update(message);
        sha256.finalize().into()
    };
    let generated_sig = sk.sign_digest_with_ecdsa(&message_hash);
    assert_eq!(hex::encode(generated_sig), expected_sig);
}

#[test]
fn should_reject_short_x_when_deserializing_private_key() {
    for short_len in 0..31 {
        let short_x = vec![42; short_len];
        assert!(PrivateKey::deserialize_sec1(&short_x).is_err());
    }
}

#[test]
fn should_reject_long_x_when_deserializing_private_key() {
    for long_len in 33..128 {
        let long_x = vec![42; long_len];
        assert!(PrivateKey::deserialize_sec1(&long_x).is_err());
    }
}

#[test]
fn generate_from_seed_is_stable() {
    let tests = [
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abcdef",
            "995da3cf545787d65f9ced52674e92ee8171c87c7a4008aa4349ec47d21609a7",
        ),
        (
            "03fc46909ddfe5ed2f37af7923d846ecab53f962a83e4fc30be550671ceab3e6",
            "37d0dc8b55b04f4b44824272d4449ebbd6363ab031c91a5cd717cbd60f3fc034",
        ),
    ];

    for (seed, expected_key) in tests {
        let sk = PrivateKey::generate_from_seed(&hex::decode(seed).unwrap());
        assert_eq!(hex::encode(sk.serialize_sec1()), expected_key);
    }
}

#[test]
fn should_accept_ecdsa_signatures_that_we_generate() {
    use rand::RngCore;

    let rng = &mut test_rng();

    let sk = PrivateKey::generate_using_rng(rng);
    let pk = sk.public_key();

    for m in 0..100 {
        let mut msg = vec![0u8; m];
        rng.fill_bytes(&mut msg);
        let sig = sk.sign_message_with_ecdsa(&msg);

        assert_eq!(
            sk.sign_message_with_ecdsa(&msg),
            sig,
            "ECDSA signature generation is deterministic"
        );

        assert!(pk.verify_ecdsa_signature(&msg, &sig));
        assert!(pk.verify_ecdsa_signature_with_malleability(&msg, &sig));
    }
}

#[test]
fn bitcoin_library_accepts_our_bip341_signatures() {
    use bitcoin::{
        hashes::hex::FromHex,
        schnorr::TapTweak,
        secp256k1::{schnorr::Signature, Message, Secp256k1, XOnlyPublicKey},
        util::taproot::TapBranchHash,
    };

    let secp256k1 = Secp256k1::new();

    let mut rng = test_rng();

    for _trial in 0..1024 {
        let sk = PrivateKey::generate_using_rng(&mut rng);

        let msg = rng.gen::<[u8; 32]>();
        let ttr = rng.gen::<[u8; 32]>();

        let sig = sk.sign_message_with_bip341(&msg, &mut rng, &ttr).unwrap();

        let pk = XOnlyPublicKey::from_slice(&sk.public_key().serialize_sec1(true)[1..]).unwrap();

        let tnh = TapBranchHash::from_hex(&hex::encode(ttr)).unwrap();

        let dk = pk.tap_tweak(&secp256k1, Some(tnh)).0.to_inner();

        let msg = Message::from_slice(&msg).unwrap();
        let sig = Signature::from_slice(&sig).unwrap();
        assert!(sig.verify(&msg, &dk).is_ok());
    }
}

#[test]
fn should_accept_bip340_signatures_that_we_generate() {
    use rand::RngCore;

    let mut rng = test_rng();

    for len in 0..100 {
        let sk = PrivateKey::generate_using_rng(&mut rng);

        let pk = sk.public_key();

        let mut msg = vec![0u8; len];
        rng.fill_bytes(&mut msg);

        let sig = sk.sign_message_with_bip340(&msg, &mut rng);
        assert!(pk.verify_bip340_signature(&msg, &sig));

        for ttr_len in [0, 32] {
            let mut ttr = vec![0u8; ttr_len];
            rng.fill_bytes(&mut ttr);
            let sig = sk.sign_message_with_bip341(&msg, &mut rng, &ttr).unwrap();
            assert!(pk.verify_bip341_signature(&msg, &sig, &ttr));
        }
    }
}

#[test]
fn should_reject_high_s_in_signature_unless_malleable() -> Result<(), KeyDecodingError> {
    let pk = PublicKey::deserialize_sec1(&hex::decode("04E38257CE81AB62AB1DF591E360AB0021D2D24E737299CF48317DBF31A3996A2A78DD07EA1996F24FE829B4EE968BA2700632D8F165E793E41AE37B8911FC83C9").unwrap())?;
    let msg = b"test";
    let sig = hex::decode("6471F8E5E63D6055AA6F6D3A8EBF49935D1316D6A54B9B09465B3BEB38E3AC14CE0FFBABD8E3248BEEBD568DCBCC7861126B1AB88E721D0206E9D67ECD878C7C").unwrap();

    assert!(!pk.verify_ecdsa_signature(msg, &sig));
    assert!(pk.verify_ecdsa_signature_with_malleability(msg, &sig));

    // Test again using the pre-hashed variants:
    let msg_hash: [u8; 32] = {
        use sha2::Digest;
        let mut sha256 = sha2::Sha256::new();
        sha256.update(msg);
        sha256.finalize().into()
    };

    assert!(!pk.verify_ecdsa_signature_prehashed(&msg_hash, &sig));
    assert!(pk.verify_ecdsa_signature_prehashed_with_malleability(&msg_hash, &sig));

    Ok(())
}

#[test]
fn should_reject_invalid_public_keys() {
    struct InvalidKey {
        reason: &'static str,
        key: Vec<u8>,
    }

    impl InvalidKey {
        fn new(reason: &'static str, key_hex: &'static str) -> Self {
            let key = hex::decode(key_hex).expect("Invalid key_hex param");
            Self { reason, key }
        }
    }

    let invalid_keys = [
        InvalidKey::new("empty", ""),
        InvalidKey::new("too short", "02"),
        InvalidKey::new(
            "valid compressed point with uncompressed header",
            "04F599CDA3A05987498A716E820651AC96A4EEAA3AD9B7D6F244A83CC3381CABC4",
        ),
        InvalidKey::new(
            "invalid x, header 02",
            "02F599CDA3A05987498A716E820651AC96A4EEAA3AD9B7D6F244A83CC3381CABC3",
        ),
        InvalidKey::new(
            "invalid x, header 03",
            "03F599CDA3A05987498A716E820651AC96A4EEAA3AD9B7D6F244A83CC3381CABC3",
        ),
        InvalidKey::new(
            "valid uncompressed point with header 02",
            "02F599CDA3A05987498A716E820651AC96A4EEAA3AD9B7D6F244A83CC3381CABC4C300A1369821A5A86D4D9BA74FF68817C4CAEA4BAC737A7B00A48C4835F28DB4"
        ),
        InvalidKey::new(
            "valid uncompressed point with header 03",
            "03F599CDA3A05987498A716E820651AC96A4EEAA3AD9B7D6F244A83CC3381CABC4C300A1369821A5A86D4D9BA74FF68817C4CAEA4BAC737A7B00A48C4835F28DB4"
        ),
        InvalidKey::new(
            "invalid uncompressed point (y off by one)",
            "04F599CDA3A05987498A716E820651AC96A4EEAA3AD9B7D6F244A83CC3381CABC4C300A1369821A5A86D4D9BA74FF68817C4CAEA4BAC737A7B00A48C4835F28DB5"
        ),
        InvalidKey::new(
            "valid P256 point",
            "04EB2D21CD969E68C767B091E91900863E7699826C3466F15B956BBB6CBAEDB09A5A16ED621975EC1BCB81A41EE5DCF719021B12A95CC858A735A266135EFD2E4E"
        ),
    ];

    for invalid_key in &invalid_keys {
        let result = PublicKey::deserialize_sec1(&invalid_key.key);

        assert!(
            result.is_err(),
            "Accepted invalid key ({})",
            invalid_key.reason
        );
    }
}

#[test]
fn should_serialization_and_deserialization_round_trip_for_private_keys(
) -> Result<(), KeyDecodingError> {
    let rng = &mut test_rng();

    for _ in 0..200 {
        let key = PrivateKey::generate_using_rng(rng);

        let key_via_sec1 = PrivateKey::deserialize_sec1(&key.serialize_sec1())?;
        let key_via_5915_der = PrivateKey::deserialize_rfc5915_der(&key.serialize_rfc5915_der())?;
        let key_via_5915_pem = PrivateKey::deserialize_rfc5915_pem(&key.serialize_rfc5915_pem())?;
        let key_via_p8_der = PrivateKey::deserialize_pkcs8_der(&key.serialize_pkcs8_der())?;
        let key_via_p8_pem = PrivateKey::deserialize_pkcs8_pem(&key.serialize_pkcs8_pem())?;

        let expected = key.serialize_sec1();
        assert_eq!(expected.len(), 32);

        assert_eq!(key_via_sec1.serialize_sec1(), expected);
        assert_eq!(key_via_5915_der.serialize_sec1(), expected);
        assert_eq!(key_via_5915_pem.serialize_sec1(), expected);
        assert_eq!(key_via_p8_der.serialize_sec1(), expected);
        assert_eq!(key_via_p8_pem.serialize_sec1(), expected);
    }
    Ok(())
}

#[test]
fn should_serialization_and_deserialization_round_trip_for_public_keys(
) -> Result<(), KeyDecodingError> {
    let rng = &mut test_rng();

    for _ in 0..2000 {
        let key = PrivateKey::generate_using_rng(rng).public_key();

        let key_via_sec1 = PublicKey::deserialize_sec1(&key.serialize_sec1(false))?;
        let key_via_sec1c = PublicKey::deserialize_sec1(&key.serialize_sec1(true))?;
        let key_via_der = PublicKey::deserialize_der(&key.serialize_der())?;
        let key_via_pem = PublicKey::deserialize_pem(&key.serialize_pem())?;

        assert_eq!(key.serialize_sec1(true).len(), 33);
        let expected = key.serialize_sec1(false);
        assert_eq!(expected.len(), 65);

        assert_eq!(key_via_sec1.serialize_sec1(false), expected);
        assert_eq!(key_via_sec1c.serialize_sec1(false), expected);
        assert_eq!(key_via_der.serialize_sec1(false), expected);
        assert_eq!(key_via_pem.serialize_sec1(false), expected);

        // This only holds for keys with an even y coordinate
        if key.serialize_sec1(false)[0] == 0x02 {
            let key_via_bip340 = PublicKey::deserialize_bip340(&key.serialize_bip340())?;
            assert_eq!(key_via_bip340.serialize_sec1(false), expected);
        }
    }

    Ok(())
}

#[test]
fn should_match_bip340_reference_test_signatures() {
    struct Bip340Test {
        msg: Vec<u8>,
        sig: Vec<u8>,
        pk: Vec<u8>,
        accept: bool,
    }

    impl Bip340Test {
        fn new(pk: &'static str, msg: &'static str, sig: &'static str, accept: bool) -> Self {
            let pk = hex::decode(pk).unwrap();
            let msg = hex::decode(msg).unwrap();
            let sig = hex::decode(sig).unwrap();
            Self {
                pk,
                msg,
                sig,
                accept,
            }
        }
    }

    let bip340_tests = [
        Bip340Test::new("F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9", "0000000000000000000000000000000000000000000000000000000000000000", "E907831F80848D1069A5371B402410364BDF1C5F8307B0084C55F1CE2DCA821525F66A4A85EA8B71E482A74F382D2CE5EBEEE8FDB2172F477DF4900D310536C0", true),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "6896BD60EEAE296DB48A229FF71DFE071BDE413E6D43F917DC8DCF8C78DE33418906D11AC976ABCCB20B091292BFF4EA897EFCB639EA871CFA95F6DE339E4B0A", true),
        Bip340Test::new("DD308AFEC5777E13121FA72B9CC1B7CC0139715309B086C960E18FD969774EB8", "7E2D58D8B3BCDF1ABADEC7829054F90DDA9805AAB56C77333024B9D0A508B75C", "5831AAEED7B44BB74E5EAB94BA9D4294C49BCF2A60728D8B4C200F50DD313C1BAB745879A5AD954A72C45A91C3A51D3C7ADEA98D82F8481E0E1E03674A6F3FB7", true),
        Bip340Test::new("25D1DFF95105F5253C4022F628A996AD3A0D95FBF21D468A1B33F8C160D8F517", "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", "7EB0509757E246F19449885651611CB965ECC1A187DD51B64FDA1EDC9637D5EC97582B9CB13DB3933705B32BA982AF5AF25FD78881EBB32771FC5922EFC66EA3", true),
        Bip340Test::new("D69C3509BB99E412E68B0FE8544E72837DFA30746D8BE2AA65975F29D22DC7B9", "4DF3C3F68FCC83B27E9D42C90431A72499F17875C81A599B566C9889B9696703", "00000000000000000000003B78CE563F89A0ED9414F5AA28AD0D96D6795F9C6376AFB1548AF603B3EB45C9F8207DEE1060CB71C04E80F593060B07D28308D7F4", true),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "FFF97BD5755EEEA420453A14355235D382F6472F8568A18B2F057A14602975563CC27944640AC607CD107AE10923D9EF7A73C643E166BE5EBEAFA34B1AC553E2", false),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "1FA62E331EDBC21C394792D2AB1100A7B432B013DF3F6FF4F99FCB33E0E1515F28890B3EDB6E7189B630448B515CE4F8622A954CFE545735AAEA5134FCCDB2BD", false),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "6CFF5C3BA86C69EA4B7376F31A9BCB4F74C1976089B2D9963DA2E5543E177769961764B3AA9B2FFCB6EF947B6887A226E8D7C93E00C5ED0C1834FF0D0C2E6DA6", false),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "0000000000000000000000000000000000000000000000000000000000000000123DDA8328AF9C23A94C1FEECFD123BA4FB73476F0D594DCB65C6425BD186051", false),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "00000000000000000000000000000000000000000000000000000000000000017615FBAF5AE28864013C099742DEADB4DBA87F11AC6754F93780D5A1837CF197", false),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "4A298DACAE57395A15D0795DDBFD1DCB564DA82B0F269BC70A74F8220429BA1D69E89B4C5564D00349106B8497785DD7D1D713A8AE82B32FA79D5F7FC407D39B", false),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F69E89B4C5564D00349106B8497785DD7D1D713A8AE82B32FA79D5F7FC407D39B", false),
        Bip340Test::new("DFF1D77F2A671C5F36183726DB2341BE58FEAE1DA2DECED843240F7B502BA659", "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89", "6CFF5C3BA86C69EA4B7376F31A9BCB4F74C1976089B2D9963DA2E5543E177769FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141", false),
    ];

    for tv in bip340_tests {
        let pk = PublicKey::deserialize_bip340(&tv.pk).unwrap();
        assert_eq!(pk.verify_bip340_signature(&tv.msg, &tv.sig), tv.accept);
    }
}

#[test]
fn should_be_able_to_parse_openssl_rfc5915_format_key() {
    pub const SAMPLE_SECP256K1_PEM: &str = r#"-----BEGIN EC PRIVATE KEY-----
MHQCAQEEIJQhkGfs2ep0VGU5BgJvcc4NVWG0GCc+aqkH7b3DL6aZoAcGBSuBBAAK
oUQDQgAENBexvaA6VKI60UxeTDHiocVBcf+y/irJOHzvQSlwiZM3MCDu6lxaP/Bw
i389XZmdlKFbsLkUI9dDQgMP98YnUA==
-----END EC PRIVATE KEY-----
"#;

    let key = PrivateKey::deserialize_rfc5915_pem(SAMPLE_SECP256K1_PEM).unwrap();

    assert_eq!(
        hex::encode(key.serialize_sec1()),
        "94219067ecd9ea7454653906026f71ce0d5561b418273e6aa907edbdc32fa699"
    );

    // Our re-encoding includes carriage returns, ignore that:
    assert_eq!(
        key.serialize_rfc5915_pem().replace('\r', ""),
        SAMPLE_SECP256K1_PEM
    );
}

#[test]
fn should_match_slip10_derivation_test_data() {
    // Test data from https://github.com/satoshilabs/slips/blob/master/slip-0010.md#test-vector-1-for-secp256k1
    let chain_code = hex!("04466b9cc8e161e966409ca52986c584f07e9dc81f735db683c3ff6ec7b1503f");

    let private_key = PrivateKey::deserialize_sec1(&hex!(
        "cbce0d719ecf7431d88e6a89fa1483e02e35092af60c042b1df2ff59fa424dca"
    ))
    .expect("Test has valid key");

    let public_key = PublicKey::deserialize_sec1(&hex!(
        "0357bfe1e341d01c69fe5654309956cbea516822fba8a601743a012a7896ee8dc2"
    ))
    .expect("Test has valid key");

    assert_eq!(
        hex::encode(public_key.serialize_sec1(true)),
        hex::encode(private_key.public_key().serialize_sec1(true))
    );

    let path = DerivationPath::new_bip32(&[2, 1000000000]);

    let (derived_secret_key, sk_chain_code) =
        private_key.derive_subkey_with_chain_code(&path, &chain_code);

    let (derived_public_key, pk_chain_code) =
        public_key.derive_subkey_with_chain_code(&path, &chain_code);
    assert_eq!(
        hex::encode(sk_chain_code),
        "c783e67b921d2beb8f6b389cc646d7263b4145701dadd2161548a8b078e65e9e"
    );
    assert_eq!(
        hex::encode(pk_chain_code),
        "c783e67b921d2beb8f6b389cc646d7263b4145701dadd2161548a8b078e65e9e"
    );

    assert_eq!(
        hex::encode(derived_public_key.serialize_sec1(true)),
        "022a471424da5e657499d1ff51cb43c47481a03b1e77f951fe64cec9f5a48f7011"
    );

    assert_eq!(
        hex::encode(derived_secret_key.serialize_sec1()),
        "471b76e389e528d6de6d816857e012c5455051cad6660850e58372a6c3e6e7c8"
    );

    assert_eq!(
        hex::encode(derived_public_key.serialize_sec1(true)),
        hex::encode(derived_secret_key.public_key().serialize_sec1(true)),
        "Derived keys match"
    );
}

#[test]
fn should_handle_short_len_prehashed() {
    // k256 somewhat arbitrarily rejects prehashed digests under 128
    // bits. This is somewhat ok, since we hopefully don't ever do
    // this, but it makes an otherwise infalliable function fallible,
    // which is unfortunate. So we perform the (correct/standard)
    // prefixing of zero padding the digest in order to make the
    // function infalliable. Test this using a short input generated
    // by another ECDSA implementation

    let pk = PublicKey::deserialize_sec1(&hex!(
        "0374558eb18c338e6116fbd147eba139210774240dcc7dbc450423fc1b0e505d8e"
    ))
    .expect("Invalid key");

    let prehash = hex!("2F45495C63D9BD3BD436D855");

    let sig = hex!("307B5A1D99434C89F243BF2678EF969FD24A85BC3B62CFD0E083715FA91879FD918BCF8FAB5F622713284C42A73D5F96CAAE4BD94BC69655A43F18FB1DF89039");

    assert!(pk.verify_ecdsa_signature_prehashed_with_malleability(&prehash, &sig));
}

#[test]
fn key_derivation_matches_bip32() {
    // SLIP10 differs from BIP32 only in the case that a 256-bit HMAC
    // output is greater than the group order. For secp256k1 this occurs
    // with the cryptographically negligible case of 1/2**127.

    let rng = &mut test_rng();

    // zeros the high bit to avoid requesting hardened derivation, which we do not support
    let path = (0..255)
        .map(|_| rng.gen::<u32>() & 0x7FFFFFFF)
        .collect::<Vec<u32>>();

    let master_key = PrivateKey::generate_using_rng(rng).public_key();
    let root_chain_code = [0u8; 32];

    let mut derived_keys = Vec::with_capacity(path.len());
    for i in 1..=path.len() {
        derived_keys.push(
            master_key
                .derive_subkey(&DerivationPath::new_bip32(&path[..i]))
                .0,
        );
    }

    let attrs = bip32::ExtendedKeyAttrs {
        depth: 0,
        parent_fingerprint: [0u8; 4],
        child_number: bip32::ChildNumber(0),
        chain_code: root_chain_code,
    };

    let ext = bip32::ExtendedKey {
        prefix: bip32::Prefix::XPUB,
        attrs,
        key_bytes: master_key
            .serialize_sec1(true)
            .try_into()
            .expect("Unexpected size"),
    };

    let bip32_mk = bip32::XPub::try_from(ext).expect("Failed to accept BIP32");

    let mut bip32_state = bip32_mk.clone();
    for (i, p) in path.iter().enumerate() {
        let derived = bip32_state
            .derive_child(bip32::ChildNumber(*p))
            .expect("Failed to derive child");
        assert_eq!(
            derived.to_bytes().to_vec(),
            derived_keys[i].serialize_sec1(true)
        );
        bip32_state = derived;
    }
}

mod try_recovery_from_digest {
    use crate::test_rng;
    use ic_crypto_secp256k1::{PrivateKey, PublicKey, RecoveryError};
    use k256::ecdsa::{Signature, VerifyingKey};
    use rand::Rng;

    #[test]
    fn should_fail_when_signature_not_parsable() {
        let rng = &mut test_rng();
        let public_key = PrivateKey::generate_using_rng(rng).public_key();

        let recid = public_key.try_recovery_from_digest(&[0], &[0_u8; 64]);

        assert_eq!(
            recid,
            Err(RecoveryError::SignatureParseError(
                "signature error".to_string()
            ))
        );
    }

    #[test]
    fn should_recover_public_key_from_y_parity() {
        let rng = &mut test_rng();
        let private_key = PrivateKey::generate_using_rng(rng);
        let public_key = private_key.public_key();
        let digest = rng.gen::<[u8; 32]>();
        let signature = private_key.sign_digest_with_ecdsa(&digest);

        let recid = public_key
            .try_recovery_from_digest(&digest, &signature)
            .expect("cannot fail because params are correct");

        let recovered_public_key = VerifyingKey::recover_from_prehash(
            &digest,
            &Signature::from_slice(&signature).expect("valid signature"),
            k256::ecdsa::RecoveryId::from_byte(recid.to_byte()).expect("valid recovery id"),
        )
        .expect("cannot fail because params are correct");

        assert_eq!(
            public_key.serialize_sec1(false),
            recovered_public_key
                .to_encoded_point(false)
                .as_bytes()
                .to_vec()
        );
    }

    #[test]
    fn should_compute_signature_y_parity_on_eth_transaction() {
        //https://sepolia.etherscan.io/tx/0x66a9a218ea720ac6d2c9e56f7e44836c1541c186b7627bda220857ce34e2df7f
        let public_key = "040b6fb17608bd3389242d3746988c6e45e89387ba80862414052a7893422397d63d52f93565eb26aebfc1d83e4d6d9fcedf08d04c91709c10d8703bfd1514811e";
        let unsigned_tx_hash = "0x2d9e6453d9864cff7453ca35dcab86be744c641ba4891c2fe9aeaa2f767b9758";
        let r = "0x7d097b81dc8bf5ad313f8d6656146d4723d0e6bb3fb35f1a709e6a3d4426c0f3";
        let s = "0x4f8a618d959e7d96e19156f0f5f2ed321b34e2004a0c8fdb7f02bc7d08b74441";
        let expected_v = true;

        let public_key =
            PublicKey::deserialize_sec1(&hex::decode(public_key).expect("valid hex string"))
                .expect("valid public key");
        let digest = hex::decode(&unsigned_tx_hash[2..]).expect("valid hex string");
        let signature = [
            hex::decode(&r[2..]).expect("valid hex string"),
            hex::decode(&s[2..]).expect("valid hex string"),
        ]
        .concat();

        let recid = public_key
            .try_recovery_from_digest(&digest, &signature)
            .expect("valid signature");

        assert_eq!(recid.is_y_odd(), expected_v);
        assert!(!recid.is_x_reduced());
    }
}
