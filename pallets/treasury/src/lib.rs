// Forked from https://github.com/paritytech/substrate/tree/master/frame/treasury

// Copyright 2021 LISTEN Developer.
// This file is part of LISTEN

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

#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
#[cfg(test)]
mod tests;

pub mod weights;

pub use crate::pallet::*;
use codec::{Decode, Encode};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, print,
	traits::{
		Currency, EnsureOrigin, ExistenceRequirement::KeepAlive, Get, Imbalance, OnUnbalanced,
		ReservableCurrency, WithdrawReasons,
	},
	weights::{DispatchClass, Weight},
	PalletId,
};
use frame_system::ensure_signed;
use listen_primitives::traits::{ListenHandler, RoomTreasuryHandler};
use pallet_listen::{self, RoomId};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{AccountIdConversion, Saturating, StaticLookup, Zero},
	Permill, RuntimeDebug, SaturatedConversion,
};
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub type ProposalIndex = u32;
pub type RoomIndex = u64;

/// A spending proposal.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct RoomTreasuryProposal<AccountId, Balance, BlockNumber> {
	/// The account proposing it.
	proposer: AccountId,
	/// The (total) amount that should be paid if the proposal is accepted.
	value: Balance,
	/// The account to whom the payment should be made if the proposal is accepted.
	beneficiary: AccountId,
	/// The amount held on deposit (reserved) for making this proposal.
	bond: Balance,
	start_spend_time: Option<BlockNumber>,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	type BalanceOf<T> = <<T as Config>::NativeCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	pub type NegativeImbalanceOf<T> = <<T as Config>::NativeCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	#[pallet::config]
	#[pallet::disable_frame_system_supertrait_check]
	pub trait Config: frame_system::Config {
		type NativeCurrency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
		type ListenHandler: ListenHandler<RoomId, Self::AccountId, DispatchError, u128>;
		/// Origin from which approvals must come.
		type ApproveOrigin: EnsureOrigin<Self::Origin>;
		/// Origin from which rejections must come.
		type RejectOrigin: EnsureOrigin<Self::Origin>;
		/// The overarching event type.
		type Event: From<Event<Self>>
			+ Into<<Self as frame_system::Config>::Event>
			+ IsType<<Self as frame_system::Config>::Event>;
		/// Handler for the unbalanced decrease when slashing for a rejected proposal or bounty.
		type OnSlash: OnUnbalanced<NegativeImbalanceOf<Self>>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
		#[pallet::constant]
		/// Fraction of a proposal's value that should be bonded in order to place the proposal.
		/// An accepted proposal gets these back. A rejected proposal does not.
		type ProposalBond: Get<Permill>;
		#[pallet::constant]
		/// Minimum amount of funds that should be placed in a deposit for making a proposal.
		type ProposalBondMinimum: Get<BalanceOf<Self>>;
		#[pallet::constant]
		/// Period between successive spends.
		type SpendPeriod: Get<Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New proposal. \[proposal_index\]
		Proposed(ProposalIndex),
		/// We have ended a spend period and will now allocate funds. \[budget_remaining\]
		Spending(BalanceOf<T>),
		/// Some funds have been allocated. \[proposal_index, award, beneficiary\]
		Awarded(ProposalIndex, BalanceOf<T>, T::AccountId),
		/// A proposal was rejected; funds were slashed. \[proposal_index, slashed\]
		Rejected(ProposalIndex, BalanceOf<T>),
		/// Some of our funds have been burnt. \[burn\]
		Burnt(BalanceOf<T>),
		/// Spending has finished; this is the amount that rolls over until next spend.
		/// \[budget_remaining\]
		Rollover(BalanceOf<T>),
		/// Some funds have been deposited. \[deposit\]
		Deposit(BalanceOf<T>),
		SpendFund(T::AccountId, RoomIndex),
	}

	/// Number of proposals that have been made.
	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub type ProposalCount<T: Config> =
		StorageMap<_, Blake2_128Concat, RoomIndex, ProposalIndex, ValueQuery>;

	/// Proposals that have been made.
	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub type Proposals<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		RoomIndex,
		Blake2_128Concat,
		ProposalIndex,
		RoomTreasuryProposal<T::AccountId, BalanceOf<T>, T::BlockNumber>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn approvals)]
	pub type Approvals<T: Config> =
		StorageMap<_, Blake2_128Concat, RoomIndex, Vec<ProposalIndex>, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Proposer's balance is too low.
		InsufficientProposersBalance,
		/// No proposal or bounty at that index.
		InvalidIndex,
		RoomHaveNoProposal,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(1500_000_000)]
		pub fn propose_spend(
			origin: OriginFor<T>,
			room_id: RoomIndex,
			#[pallet::compact] value: BalanceOf<T>,
			beneficiary: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let proposer = ensure_signed(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;

			let bond = Self::calculate_bond(value);
			T::NativeCurrency::reserve(&proposer, bond)
				.map_err(|_| Error::<T>::InsufficientProposersBalance)?;

			let c = Self::proposal_count(room_id);
			let start_spend_time = None;
			<ProposalCount<T>>::insert(room_id, c + 1);
			<Proposals<T>>::insert(
				room_id,
				c,
				RoomTreasuryProposal { proposer, value, beneficiary, bond, start_spend_time },
			);

			Self::deposit_event(Event::Proposed(c));
			Ok(())
		}

		#[pallet::weight(1500_000_000)]
		pub fn reject_proposal(
			origin: OriginFor<T>,
			room_id: RoomIndex,
			#[pallet::compact] proposal_id: ProposalIndex,
		) -> DispatchResult {
			T::RejectOrigin::ensure_origin(origin)?;

			let proposal =
				<Proposals<T>>::take(room_id, &proposal_id).ok_or(Error::<T>::InvalidIndex)?;
			let value = proposal.bond;
			let imbalance = T::NativeCurrency::slash_reserved(&proposal.proposer, value).0;
			T::OnSlash::on_unbalanced(imbalance);

			Self::deposit_event(Event::<T>::Rejected(proposal_id, value));
			Ok(())
		}

		#[pallet::weight(1500_000_000)]
		pub fn approve_proposal(
			origin: OriginFor<T>,
			room_id: RoomIndex,
			#[pallet::compact] proposal_id: ProposalIndex,
		) -> DispatchResult {
			T::ApproveOrigin::ensure_origin(origin)?;

			ensure!(<Proposals<T>>::contains_key(room_id, proposal_id), Error::<T>::InvalidIndex);

			<Proposals<T>>::mutate(room_id, proposal_id, |h| {
				if let Some(p) = h {
					p.start_spend_time = Some(Self::now() + T::SpendPeriod::get());
				}
			});

			<Approvals<T>>::mutate(room_id, |h| h.push(proposal_id));
			Ok(())
		}

		/// Users receive funds manually
		#[pallet::weight(1500_000_000)]
		pub fn spend_fund(origin: OriginFor<T>, room_id: RoomIndex) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut proposal_ids = <Approvals<T>>::get(room_id);
			if proposal_ids.len() == 0 {
				return Err(Error::<T>::RoomHaveNoProposal)?
			}

			for proposal_id in proposal_ids.clone().iter() {
				if let Some(proposal) = <Proposals<T>>::get(room_id, proposal_id) {
					if proposal.start_spend_time.is_some() &&
						proposal.start_spend_time.unwrap() <= Self::now() &&
						T::ListenHandler::sub_room_free_amount(
							room_id.into(),
							proposal.value.saturated_into::<u128>(),
						)
						.is_ok()
					{
						T::NativeCurrency::deposit_creating(&proposal.beneficiary, proposal.value);

						T::NativeCurrency::unreserve(&proposal.proposer, proposal.bond);
						proposal_ids.retain(|h| h != proposal_id);
						<Proposals<T>>::remove(room_id, proposal_id);
					}
				}
			}

			<Approvals<T>>::insert(room_id, proposal_ids);

			Self::deposit_event(Event::<T>::SpendFund(who, room_id));
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn calculate_bond(value: BalanceOf<T>) -> BalanceOf<T> {
			T::ProposalBondMinimum::get().max(T::ProposalBond::get() * value)
		}

		pub fn now() -> T::BlockNumber {
			<frame_system::Module<T>>::block_number()
		}
	}

	impl<T: Config> RoomTreasuryHandler<RoomIndex> for Pallet<T> {
		fn remove_room_treasury_info(room_id: RoomIndex) {
			<Proposals<T>>::remove_prefix(room_id, None);
			/// todo 已经同意通过的资金议案 不应该被清除
			<Approvals<T>>::remove(room_id);
		}
	}
}
