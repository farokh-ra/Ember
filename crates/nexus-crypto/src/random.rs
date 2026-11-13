//! Operating-system randomness.

use crate::error::CryptoError;

/// Fills the buffer from the operating-system CSPRNG.
pub fn fill_random(output: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(output).map_err(|_| CryptoError::Randomness)
}

/// Returns an array filled from the operating-system CSPRNG.
pub fn random_array<const N: usize>() -> Result<[u8; N], CryptoError> {
    let mut output = [0_u8; N];
    fill_random(&mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::random_array;

    #[test]
    fn random_output_is_not_constant() {
        let first: [u8; 32] = random_array().expect("randomness");
        let second: [u8; 32] = random_array().expect("randomness");
        assert_ne!(first, second);
    }
}
