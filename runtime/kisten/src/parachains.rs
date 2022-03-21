/// kico
pub mod kico {
	pub const PARA_ID: u32 = 2017;
	pub mod KICO {
		pub const AssetId: u32 = 10;
		pub const TokenSymbol: &[u8] = "KICO".as_bytes();
	}
}

/// native
pub mod native {
	pub mod KT {
		pub const AssetId: u32 = 0;
		pub const TokenSymbol: &[u8] = "KT".as_bytes();
	}
}

/// listen
pub mod listen {
	pub const PARA_ID: u32 = 1002;
	pub mod LT {
		pub const AssetId: u32 = 100;
		pub const TokenSymbol: &[u8] = "LT".as_bytes();
	}

	pub mod USDT {
		pub const AssetId: u32 = 5;
		pub const TokenSymbol: &[u8] = "USDT".as_bytes();
	}
}

/// kusama
pub mod kusama {
	pub mod KSM {
		pub const AssetId: u32 = 2;
		pub const TokenSymbol: &[u8] = "KSM".as_bytes();
	}
}

/// statemine
pub mod statemine {
	pub const PARA_ID: u32 = 1000;
}
