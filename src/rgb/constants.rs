// Default
pub const LIB_NAME_BITMASK: &str = "bitmask";
pub const RGB_CHANGE_INDEX: &str = "0";
pub const RGB_PSBT_TAPRET: &str = "TAPRET";
pub const RGB_DEFAULT_NAME: &str = "default";
pub const RGB_OLDEST_VERSION: [u8; 8] = [0; 8];
pub const RGB_STRICT_TYPE_VERSION: [u8; 8] = *b"rgbst161";
pub const RGB_DEFAULT_FETCH_LIMIT: u32 = 10;
pub const BITCOIN_DEFAULT_FETCH_LIMIT: u32 = 20;

// General Errors
#[cfg(target_arch = "wasm32")]
pub const CARBONADO_UNAVAILABLE: &str = "carbonado filesystem";
#[cfg(not(target_arch = "wasm32"))]
pub const CARBONADO_UNAVAILABLE: &str = "carbonado server";
pub const STOCK_UNAVAILABLE: &str = "Unable to access Stock data";
pub const WALLET_UNAVAILABLE: &str = "Unable to access Wallet data";
pub const TRANSFER_UNAVAILABLE: &str = "Unable to access transfer data";
