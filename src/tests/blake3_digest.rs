use super::*;

#[test]
fn consuming_finalize_and_explicit_reset_match_published_empty_vector() {
    let expected = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
    let mut hasher = Blake3Digest::default();
    Update::update(&mut hasher, b"discard this input");
    Reset::reset(&mut hasher);
    let mut output = Output::<Blake3Digest>::default();
    FixedOutput::finalize_into(hasher, &mut output);
    assert_eq!(hex::encode(output), expected);
}
use digest::Digest;
#[test]
fn finalize_reset_matches_native_blake3_and_resets_state() {
    let mut hasher = Blake3Digest::default();
    Digest::update(&mut hasher, b"data");
    assert_eq!(
        &hasher.finalize_reset()[..],
        blake3::hash(b"data").as_bytes()
    );
    assert_eq!(&hasher.finalize_reset()[..], blake3::hash(b"").as_bytes());
}
