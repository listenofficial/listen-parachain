
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;
mod benchmarking;

pub mod weights;

#[cfg(feature = "std")]
use serde::{Serialize, Deserialize};
use sp_std::prelude::*;
 use sp_runtime::SaturatedConversion;
 use listen_traits::{ListenHandler, RoomTreasuryHandler};
use frame_support::{decl_module, decl_storage, decl_event, ensure, print, decl_error, StorageDoubleMap};
use frame_support::traits::{
	Currency, Get, Imbalance, OnUnbalanced, ExistenceRequirement::{KeepAlive},
	ReservableCurrency, WithdrawReasons
};
use sp_runtime::{Permill, ModuleId, RuntimeDebug, traits::{
	Zero, StaticLookup, AccountIdConversion, Saturating
}};
use frame_support::weights::{Weight, DispatchClass};
use frame_support::traits::{EnsureOrigin};
use codec::{Encode, Decode};
use frame_system::{ensure_signed};
pub use weights::WeightInfo;
use pallet_listen;


type BalanceOf<T> = <<T as pallet_listen::Config>::NativeCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type NegativeImbalanceOf<T> =
	<<T as pallet_listen::Config>::NativeCurrency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

pub trait Config: frame_system::Config + pallet_listen::Config {

	/// Origin from which approvals must come.
	type ApproveOrigin: EnsureOrigin<Self::Origin>;

	/// Origin from which rejections must come.
	type RejectOrigin: EnsureOrigin<Self::Origin>;

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// Handler for the unbalanced decrease when slashing for a rejected proposal or bounty.
	type OnSlash: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// Fraction of a proposal's value that should be bonded in order to place the proposal.
	/// An accepted proposal gets these back. A rejected proposal does not.
	type ProposalBond: Get<Permill>;

	/// Minimum amount of funds that should be placed in a deposit for making a proposal.
	type ProposalBondMinimum: Get<BalanceOf<Self>>;

	/// Period between successive spends.
	type SpendPeriod: Get<Self::BlockNumber>;

	/// Weight information for extrinsics in this pallet.
	type WeightInfo: WeightInfo;

}

/// An index of a proposal. Just a `u32`.
pub type ProposalIndex = u32;
pub type RoomIndex = u64;

/// A spending proposal.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct Proposal<AccountId, Balance, BlockNumber> {
	/// The account proposing it.
	proposer: AccountId,
	/// The (total) amount that should be paid if the proposal is accepted.
	value: Balance,
	/// The account to whom the payment should be made if the proposal is accepted.
	beneficiary: AccountId,
	/// The amount held on deposit (reserved) for making this proposal.
	bond: Balance,
	/// 可以消费的时间
	spend_time: Option<BlockNumber>,
}

decl_storage! {
	trait Store for Module<T: Config> as Treasury {
		/// Number of proposals that have been made.
		ProposalCount get(fn proposal_count): map hasher(identity) RoomIndex => ProposalIndex;

		/// Proposals that have been made.
		pub Proposals get(fn proposals):
			double_map hasher(identity) RoomIndex, hasher(identity) ProposalIndex
			=> Option<Proposal<T::AccountId, BalanceOf<T>, T::BlockNumber>>;

		/// Proposal indices that have been approved but not yet awarded.
		pub Approvals get(fn approvals): map hasher(identity) RoomIndex => Vec<ProposalIndex>;
	}

}

decl_event!(
	pub enum Event<T>
	where
		Balance = BalanceOf<T>,
		<T as frame_system::Config>::AccountId,
	{
		/// New proposal. \[proposal_index\]
		Proposed(ProposalIndex),
		/// We have ended a spend period and will now allocate funds. \[budget_remaining\]
		Spending(Balance),
		/// Some funds have been allocated. \[proposal_index, award, beneficiary\]
		Awarded(ProposalIndex, Balance, AccountId),
		/// A proposal was rejected; funds were slashed. \[proposal_index, slashed\]
		Rejected(ProposalIndex, Balance),
		/// Some of our funds have been burnt. \[burn\]
		Burnt(Balance),
		/// Spending has finished; this is the amount that rolls over until next spend.
		/// \[budget_remaining\]
		Rollover(Balance),
		/// Some funds have been deposited. \[deposit\]
		Deposit(Balance),
		SpendFund(AccountId, RoomIndex),
	}
);

decl_error! {
	/// Error for the treasury module.
	pub enum Error for Module<T: Config> {
		/// Proposer's balance is too low.
		InsufficientProposersBalance,
		/// No proposal or bounty at that index.
		InvalidIndex,
		/// 房间没有议案
		RoomHaveNoProposal,
	}
}

decl_module! {
	pub struct Module<T: Config>
		for enum Call
		where origin: T::Origin
	{
		/// Fraction of a proposal's value that should be bonded in order to place the proposal.
		/// An accepted proposal gets these back. A rejected proposal does not.
		const ProposalBond: Permill = T::ProposalBond::get();

		/// Minimum amount of funds that should be placed in a deposit for making a proposal.
		const ProposalBondMinimum: BalanceOf<T> = T::ProposalBondMinimum::get();

		/// 议案通过多久后可以进行资金消费
		const SpendPeriod: T::BlockNumber = T::SpendPeriod::get();

		// /// The treasury's module id, used for deriving its sovereign account ID.
		// const ModuleId: ModuleId = T::ModuleId::get();

		type Error = Error<T>;

		fn deposit_event() = default;


		/// 提一个消费议案
		#[weight = 10000]
		pub fn propose_spend(
			origin,
			room_id: RoomIndex,
			#[compact] value: BalanceOf<T>,
			beneficiary: <T::Lookup as StaticLookup>::Source
		) {
			let proposer = ensure_signed(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;

			let bond = Self::calculate_bond(value);
			T::NativeCurrency::reserve(&proposer, bond)
				.map_err(|_| Error::<T>::InsufficientProposersBalance)?;

			let c = Self::proposal_count(room_id);
			let spend_time = None;
			<ProposalCount>::insert(room_id, c + 1);
			<Proposals<T>>::insert(room_id, c, Proposal { proposer, value, beneficiary, bond, spend_time });

			Self::deposit_event(RawEvent::Proposed(c));
		}


		/// 拒绝议案
		#[weight = 10000]
		pub fn reject_proposal(origin, room_id: RoomIndex, #[compact] proposal_id: ProposalIndex) {
			T::RejectOrigin::ensure_origin(origin)?;

			let proposal = <Proposals<T>>::take(room_id, &proposal_id).ok_or(Error::<T>::InvalidIndex)?;
			let value = proposal.bond;
			let imbalance = T::NativeCurrency::slash_reserved(&proposal.proposer, value).0;
			T::OnSlash::on_unbalanced(imbalance);

			Self::deposit_event(Event::<T>::Rejected(proposal_id, value));
		}


		/// 赞成议案（加入待执行队列)
		#[weight = 10000]
		pub fn approve_proposal(origin, room_id: RoomIndex, #[compact] proposal_id: ProposalIndex) {
			T::ApproveOrigin::ensure_origin(origin)?;

			ensure!(<Proposals<T>>::contains_key(room_id, proposal_id), Error::<T>::InvalidIndex);
			<Proposals<T>>::mutate(room_id, proposal_id, |h| if let Some(p) = h {
				p.spend_time = Some(<pallet_listen::Module<T>>::now() + T::SpendPeriod::get());
			});
			<Approvals>::mutate(room_id, |h| h.push(proposal_id));

		}


		/// 手动获取资金
		#[weight = 10000]
		pub fn spend_fund(origin, room_id: RoomIndex) {
			let who = ensure_signed(origin)?;
			let mut proposal_ids = <Approvals>::get(room_id);
			if proposal_ids.len() == 0 {
				return Err(Error::<T>::RoomHaveNoProposal)?;
			}

			for proposal_id in proposal_ids.clone().iter() {
				if let Some(proposal) =  <Proposals<T>>::get(room_id, proposal_id) {

					if <pallet_listen::Module<T>>::sub_room_free_amount(room_id, proposal.value.saturated_into::<u128>()).is_ok() {
						T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&proposal.beneficiary, proposal.value));
						T::NativeCurrency::reserve(&proposal.proposer, proposal.bond);
						proposal_ids.retain(|h| h != proposal_id);
						<Proposals<T>>::remove(room_id, proposal_id);
					}
				}
			}

			<Approvals>::insert(room_id, proposal_ids);

			Self::deposit_event(Event::<T>::SpendFund(who, room_id));

		}

	}
}

impl<T: Config> Module<T> {


	fn calculate_bond(value: BalanceOf<T>) -> BalanceOf<T> {
		T::ProposalBondMinimum::get().max(T::ProposalBond::get() * value)
	}

}


impl<T: Config> RoomTreasuryHandler<RoomIndex> for Module<T> {
	fn remove_room_treasury_info(room_id: RoomIndex) {
		<Proposals<T>>::remove_prefix(room_id);
		<Approvals>::remove(room_id);
	}
}

