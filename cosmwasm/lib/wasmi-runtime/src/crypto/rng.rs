use enclave_ffi_types::CryptoError;
use sgx_trts::trts::rsgx_read_rand;

pub fn rand_slice(rand: &mut [u8]) -> Result<(), CryptoError> {
    rsgx_read_rand(rand).map_err(|e| CryptoError::RandomError {})
}