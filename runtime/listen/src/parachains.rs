/// native
pub mod native {
	pub mod lt {
		pub const ASSET_ID: u32 = 0;
		pub const TOKEN_SYMBOL: &[u8] = "LT".as_bytes();
	}

	pub mod usdt {
		pub const ASSET_ID: u32 = 5;
		pub const TOKEN_SYMBOL: &[u8] = "USDT".as_bytes();
	}

	pub mod like {
		pub const ASSET_ID: u32 = 1;
		pub const TOKEN_SYMBOL: &[u8] = "LIKE".as_bytes();
	}
}

/// kusama
pub mod kusama {
	pub mod ksm {
		pub const ASSET_ID: u32 = 2;
	}
}

/// kico
pub mod kico {
	pub const PARA_ID: u32 = 2000;
	pub mod kico {
		pub const ASSET_ID: u32 = 10;
		pub const TOKEN_SYMBOL: &[u8] = "KICO".as_bytes();
	}
}

/// kisten (Test)
pub mod kisten {
	pub const PARA_ID: u32 = 2025;
	pub mod kt {
		pub const ASSET_ID: u32 = 100;
		pub const TOKEN_SYMBOL: &[u8] = "KT".as_bytes();
	}
}

/// statemine
pub mod statemine {
	pub const PARA_ID: u32 = 1000;
}
