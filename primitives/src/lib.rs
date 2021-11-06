// This file is part of Substrate.

// Copyright (C) 2018-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Low-level types used throughout the Substrate code.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

// #![cfg_attr(not(feature = "std"), no_std)]
// #![allow(clippy::unnecessary_cast)]
// #![allow(clippy::upper_case_acronyms)]

use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, OpaqueExtrinsic,
};

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::convert::TryInto;

/// An index to a block.
pub type BlockNumber = u32;

pub type Amount = i128;

pub type CurrencyId = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them.
pub type AccountIndex = u32;

// /// currency_id
// pub type CurrencyId = u32;

/// Balance of an account.
pub type Balance = u128;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// A timestamp: milliseconds since the unix epoch.
/// `u64` is enough to represent a duration of half a billion years, when the
/// time scale is milliseconds.
pub type Timestamp = u64;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;
/// Header type.
/// *************************************************************************************************
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type.
pub type Block = generic::Block<Header, OpaqueExtrinsic>;
/// Block ID.
pub type BlockId = generic::BlockId<Block>;

// /// App-specific crypto used for reporting equivocation/misbehavior in BABE and
// /// GRANDPA. Any rewards for misbehavior reporting will be paid out to this
// /// account.
// pub mod report {
// 	use super::{Signature, Verify};
// 	use frame_system::offchain::AppCrypto;
// 	use sp_core::crypto::{key_types, KeyTypeId};
//
// 	/// Key type for the reporting module. Used for reporting BABE and GRANDPA
// 	/// equivocations.
// 	// #[cfg(feature = "std")]
// 	pub const KEY_TYPE: KeyTypeId = key_types::REPORTING;
//
// 	mod app {
// 		// #[cfg(feature = "std")]
// 		use sp_application_crypto::{app_crypto, sr25519};
// 		app_crypto!(sr25519, super::KEY_TYPE);
// 	}
//
// 	/// Identity of the equivocation/misbehavior reporter.
// 	// #[cfg(feature = "std")]
// 	pub type ReporterId = app::Public;
//
// 	/// An `AppCrypto` type to allow submitting signed transactions using the reporting
// 	/// application key as signer.
// 	pub struct ReporterAppCrypto;
//
// 	impl AppCrypto<<Signature as Verify>::Signer, Signature> for ReporterAppCrypto {
// 		type RuntimeAppPublic = ReporterId;
// 		type GenericSignature = sp_core::sr25519::Signature;
// 		type GenericPublic = sp_core::sr25519::Public;
// 	}
// }

// #[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
// pub enum Tokens {
// 	LT,
// 	KSM,
// 	DOT,
// 	BTC,
// 	ACA,
// 	Other(CurrencyId),
// }
//
// impl Default for Tokens {
// 	fn default() -> Self {
// 		Self::LT
// 	}
// }
//
// impl TryInto<CurrencyId> for Tokens {
// 	type Error = &'static str;
//
// 	fn try_into(self) -> Result<CurrencyId, Self::Error> {
// 		match self {
// 			Tokens::LT => Ok(0 as CurrencyId),
// 			Tokens::BTC => Ok(1 as CurrencyId),
// 			Tokens::DOT => Ok(2 as CurrencyId),
// 			Tokens::KSM => Ok(3 as CurrencyId),
// 			Tokens::ACA => Ok(4 as CurrencyId),
// 			Tokens::Other(x) => Ok(x as CurrencyId),
// 			_ => Err("unexpect token"),
// 	}
// }
// }
