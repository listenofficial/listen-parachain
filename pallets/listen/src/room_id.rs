#![cfg_attr(not(feature = "std"), no_std)]
use codec::{self, Decode, Encode};
use scale_info::TypeInfo;
use sp_std::prelude::*;

#[derive(Decode, Encode, Copy, Clone, Default, PartialEq, Debug, TypeInfo)]
pub struct RoomTreasuryId(pub u64);

impl From<RoomTreasuryId> for u64 {
	fn from(x: RoomTreasuryId) -> Self {
		x.0
	}
}

impl From<u64> for RoomTreasuryId {
	fn from(x: u64) -> Self {
		RoomTreasuryId(x)
	}
}

/// This type can be converted into and possibly from an [`AccountId`] (which itself is generic).
pub trait AccountIdConversion<AccountId>: Sized {
	/// Convert into an account ID. This is infallible.
	fn into_account(&self) -> AccountId;

	/// Try to convert an account ID into this type. Might not succeed.
	fn try_from_account(a: &AccountId) -> Option<Self>;
}

impl<T: Encode + Decode> AccountIdConversion<T> for RoomTreasuryId {
	fn into_account(&self) -> T {
		(b"room", self).using_encoded(|b| T::decode(&mut TrailingZeroInput(b))).unwrap()
	}

	fn try_from_account(x: &T) -> Option<Self> {
		x.using_encoded(|d| {
			if &d[0..4] != b"room" {
				return None
			}
			let mut cursor = &d[4..];
			let result = Decode::decode(&mut cursor).ok()?;
			if cursor.iter().all(|x| *x == 0) {
				Some(result)
			} else {
				None
			}
		})
	}
}

struct TrailingZeroInput<'a>(&'a [u8]);
impl<'a> codec::Input for TrailingZeroInput<'a> {
	fn remaining_len(&mut self) -> Result<Option<usize>, codec::Error> {
		Ok(None)
	}

	fn read(&mut self, into: &mut [u8]) -> Result<(), codec::Error> {
		let len = into.len().min(self.0.len());
		into[..len].copy_from_slice(&self.0[..len]);
		for i in &mut into[len..] {
			*i = 0;
		}
		self.0 = &self.0[len..];
		Ok(())
	}
}
