use digest::{FixedOutput, FixedOutputReset, HashMarker, Output, OutputSizeUser, Reset, Update};

/// Bridge the stable BLAKE3 API to the digest version used by other algorithms.
/// BLAKE3's optional traits-preview feature may change digest versions.
#[derive(Default)]
pub struct Blake3Digest(blake3::Hasher);

impl OutputSizeUser for Blake3Digest {
    type OutputSize = digest::consts::U32;
}
impl HashMarker for Blake3Digest {}
impl Update for Blake3Digest {
    fn update(&mut self, input: &[u8]) {
        self.0.update(input);
    }
}
impl FixedOutput for Blake3Digest {
    fn finalize_into(self, output: &mut Output<Self>) {
        output.copy_from_slice(self.0.finalize().as_bytes());
    }
}
impl Reset for Blake3Digest {
    fn reset(&mut self) {
        self.0.reset();
    }
}
impl FixedOutputReset for Blake3Digest {
    fn finalize_into_reset(&mut self, output: &mut Output<Self>) {
        output.copy_from_slice(self.0.finalize().as_bytes());
        self.0.reset();
    }
}

#[cfg(test)]
#[path = "tests/blake3_digest.rs"]
mod tests;
