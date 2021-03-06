

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]

use sp_std::{prelude::*, result, collections::{btree_set::BTreeSet, btree_map::BTreeMap} };
use sp_core::u32_trait::Value as U32;
use sp_io::storage;
use sp_std::convert::From;
use sp_runtime::{RuntimeDebug, traits::Hash};
use listen_traits::{ListenHandler, CollectiveHandler};

use frame_support::{
	codec::{Decode, Encode},
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::{
		DispatchError, DispatchResult, DispatchResultWithPostInfo, Dispatchable, Parameter,
		PostDispatchInfo,
	},
	ensure,
	traits::{ChangeMembers, EnsureOrigin, Get, InitializeMembers, Currency, ReservableCurrency},
	weights::{DispatchClass, GetDispatchInfo, Weight, Pays},
};
use frame_system::{self as system, ensure_signed, ensure_root};
use pallet_timestamp;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

/// Simple index type for proposal counting.
pub type ProposalIndex = u32;

pub type RoomIndex = u64;

/// A number of members.
///
/// This also serves as a number of voting members, and since for motions, each member may
/// vote exactly once, therefore also the number of votes for any given motion.
pub type MemberCount = u32;

/// Default voting strategy when a member is inactive.
pub trait DefaultVote {
	/// Get the default voting strategy, given:
	///
	/// - Whether the prime member voted Aye.
	/// - Raw number of yes votes.
	/// - Raw number of no votes.
	/// - Total number of member count.
	fn default_vote(
		prime_vote: Option<bool>,
		yes_votes: MemberCount,
		no_votes: MemberCount,
		len: MemberCount,
	) -> bool;
}

/// Set the prime member's vote as the default vote.
pub struct PrimeDefaultVote;

impl DefaultVote for PrimeDefaultVote {
	fn default_vote(
		prime_vote: Option<bool>,
		_yes_votes: MemberCount,
		_no_votes: MemberCount,
		_len: MemberCount,
	) -> bool {
		prime_vote.unwrap_or(false)
	}
}

/// First see if yes vote are over majority of the whole collective. If so, set the default vote
/// as yes. Otherwise, use the prime meber's vote as the default vote.
pub struct MoreThanMajorityThenPrimeDefaultVote;

impl DefaultVote for MoreThanMajorityThenPrimeDefaultVote {
	fn default_vote(
		prime_vote: Option<bool>,
		yes_votes: MemberCount,
		_no_votes: MemberCount,
		len: MemberCount,
	) -> bool {
		let more_than_majority = yes_votes * 2 > len;
		more_than_majority || prime_vote.unwrap_or(false)
	}
}

pub trait Config<I: Instance=DefaultInstance>: frame_system::Config {
	/// The outer origin type.
	type Origin: From<RawOrigin<Self::AccountId, I>>;

	/// The outer call dispatch type.
	type Proposal: Parameter
		+ Dispatchable<Origin=<Self as Config<I>>::Origin, PostInfo=PostDispatchInfo>
		+ From<frame_system::Call<Self>>
		+ GetDispatchInfo;

	/// The outer event type.
	type Event: From<Event<Self, I>> + Into<<Self as frame_system::Config>::Event>;

	/// The time-out for council motions.
	type MotionDuration: Get<Self::BlockNumber>;

	/// Maximum number of proposals allowed to be active in parallel.
	type MaxProposals: Get<ProposalIndex>;

	/// The maximum number of members supported by the pallet. Used for weight estimation.
	///
	/// NOTE:
	/// + Benchmarks will need to be re-run and weights adjusted if this changes.
	/// + This pallet assumes that dependents keep to the limit without enforcing it.
	type MaxMembers: Get<MemberCount>;

	/// Default vote strategy of this collective.
	type DefaultVote: DefaultVote;

	/// Weight information for extrinsics in this pallet.
	type WeightInfo: WeightInfo;

	type ListenHandler: ListenHandler<RoomIndex, Self::AccountId, DispatchError, u128>;
}


/// Origin for the collective module.
#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode)]
pub enum RawOrigin<AccountId, I> {
	/// It has been condoned by a given number of members of the collective from a given total.
	Members(MemberCount, MemberCount),
	/// It has been condoned by a single member of the collective.
	Member(RoomIndex, AccountId),
	/// Dummy to manage the fact we have instancing.
	_Phantom(sp_std::marker::PhantomData<I>),
}

/// Origin for the collective module.
pub type Origin<T, I=DefaultInstance> = RawOrigin<<T as frame_system::Config>::AccountId, I>;

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
/// Info for keeping track of a motion being voted on.
pub struct RoomCollectiveVotes<AccountId, BlockNumber> {
	/// The proposal's unique index.
	index: ProposalIndex,
	/// The proposal's reason,
	reason: Option<Vec<u8>>,
	/// The number of approval RoomCollectiveVotes that are needed to pass the motion.
	threshold: MemberCount,
	/// The current set of voters that approved it.
	ayes: Vec<AccountId>,
	/// The current set of voters that rejected it.
	nays: Vec<AccountId>,
	/// The hard end time of this vote.
	end: BlockNumber,
}

decl_storage! {
	trait Store for Module<T: Config<I>, I: Instance=DefaultInstance> as Collective {
		/// The hashes of the active proposals.
		pub Proposals get(fn proposals): map hasher(identity) RoomIndex => Vec<T::Hash>;

		/// Actual proposal for a given hash, if it's current.
		pub ProposalOf get(fn proposal_of):
			double_map hasher(identity) RoomIndex, hasher(identity) T::Hash => Option<<T as Config<I>>::Proposal>;
		/// Votes on a given proposal, if it is ongoing.
		pub Voting get(fn voting):
			double_map hasher(identity) RoomIndex, hasher(identity) T::Hash => Option<RoomCollectiveVotes<T::AccountId, T::BlockNumber>>;
		/// Proposals so far.
		pub ProposalCount get(fn proposal_count): map hasher(identity) RoomIndex => u32;

	}
	// add_extra_genesis {
	// 	config(phantom): sp_std::marker::PhantomData<I>;
	// 	config(members): Vec<T::AccountId>;
	// 	build(|config| Module::<T, I>::initialize_members(&config.members))
	// }
}

decl_event! {
	pub enum Event<T, I=DefaultInstance> where
		<T as frame_system::Config>::Hash,
		<T as frame_system::Config>::AccountId,
	{
		/// A motion (given hash) has been proposed (by given account) with a threshold (given
		/// `MemberCount`).
		/// \[account, proposal_index, proposal_hash, threshold\]
		Proposed(AccountId, ProposalIndex, Hash, MemberCount),
		/// A motion (given hash) has been voted on by given account, leaving
		/// a tally (yes votes and no votes given respectively as `MemberCount`).
		/// \[account, proposal_hash, voted, yes, no\]
		Voted(AccountId, Hash, bool, MemberCount, MemberCount),
		/// A motion was approved by the required threshold.
		/// \[proposal_hash\]
		Approved(Hash),
		/// A motion was not approved by the required threshold.
		/// \[proposal_hash\]
		Disapproved(Hash),
		/// A motion was executed; result will be `Ok` if it returned without error.
		/// \[proposal_hash, result\]
		Executed(Hash, DispatchResult),
		/// A single member did some action; result will be `Ok` if it returned without error.
		/// \[proposal_hash, result\]
		MemberExecuted(Hash, DispatchResult),
		/// A proposal was closed because its threshold was reached or after its duration was up.
		/// \[proposal_hash, yes, no\]
		Closed(Hash, MemberCount, MemberCount),
	}
}

decl_error! {
	pub enum Error for Module<T: Config<I>, I: Instance> {
		/// Account is not a member
		NotMember,
		/// Duplicate proposals not allowed
		DuplicateProposal,
		/// Proposal must exist
		ProposalMissing,
		/// Mismatched index
		WrongIndex,
		/// Duplicate vote ignored
		DuplicateVote,
		/// Members are already initialized!
		AlreadyInitialized,
		/// The close call was made too early, before the end of the voting.
		TooEarly,
		/// There can only be a maximum of `MaxProposals` active proposals.
		TooManyProposals,
		/// The given weight bound for the proposal was too low.
		WrongProposalWeight,
		/// The given length bound for the proposal was too low.
		WrongProposalLength,

	}
}

/// Return the weight of a dispatch call result as an `Option`.
///
/// Will return the weight regardless of what the state of the result is.
fn get_result_weight(result: DispatchResultWithPostInfo) -> Option<Weight> {
	match result {
		Ok(post_info) => post_info.actual_weight,
		Err(err) => err.post_info.actual_weight,
	}
}


// Note that councillor operations are assigned to the operational class.
decl_module! {
	pub struct Module<T: Config<I>, I: Instance=DefaultInstance> for enum Call where origin: <T as frame_system::Config>::Origin {
		type Error = Error<T, I>;

		fn deposit_event() = default;

		#[weight = (
			T::WeightInfo::execute(
				*length_bound, // B
				T::MaxMembers::get(), // M
			).saturating_add(proposal.get_dispatch_info().weight), // P
			DispatchClass::Operational
		)]
		fn execute(origin,
			room_id: RoomIndex,
			proposal: Box<<T as Config<I>>::Proposal>,
			#[compact] length_bound: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let members = T::ListenHandler::get_room_council(room_id)?;

			let room_owner = T::ListenHandler::get_root(room_id)?;

			ensure!(members.contains(&who) || room_owner == who.clone(), Error::<T, I>::NotMember);

			let proposal_len = proposal.using_encoded(|x| x.len());

			ensure!(proposal_len <= length_bound as usize, Error::<T, I>::WrongProposalLength);

			let proposal_hash = T::Hashing::hash_of(&proposal);

			let result = proposal.dispatch(RawOrigin::Member(room_id, who).into());

			Self::deposit_event(
				RawEvent::MemberExecuted(proposal_hash, result.map(|_| ()).map_err(|e| e.error))
			);

			Ok(get_result_weight(result).map(|w| {
				T::WeightInfo::execute(
					proposal_len as u32,  // B
					members.len() as u32, // M
				).saturating_add(w) // P
			}).into())
		}


		#[weight = (
			if *threshold < 2 {
				T::WeightInfo::propose_execute(
					*length_bound, // B
					T::MaxMembers::get(), // M
				).saturating_add(proposal.get_dispatch_info().weight) // P1
			} else {
				T::WeightInfo::propose_proposed(
					*length_bound, // B
					T::MaxMembers::get(), // M
					T::MaxProposals::get(), // P2
				)
			},
			DispatchClass::Operational
		)]

		fn propose(origin,
			room_id: RoomIndex,
			#[compact] threshold: MemberCount,
			proposal: Box<<T as Config<I>>::Proposal>,
			reason: Option<Vec<u8>>,
			#[compact] length_bound: u32
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let members = T::ListenHandler::get_room_council(room_id)?;
			ensure!(members.contains(&who), Error::<T, I>::NotMember);

			let proposal_len = proposal.using_encoded(|x| x.len());
			ensure!(proposal_len <= length_bound as usize, Error::<T, I>::WrongProposalLength);
			let proposal_hash = T::Hashing::hash_of(&proposal);
			ensure!(!<ProposalOf<T, I>>::contains_key(room_id, proposal_hash), Error::<T, I>::DuplicateProposal);

			if threshold < 2 {
				let seats = members.len() as MemberCount;
				let result = proposal.dispatch(RawOrigin::Members(1, seats).into());
				Self::deposit_event(
					RawEvent::Executed(proposal_hash, result.map(|_| ()).map_err(|e| e.error))
				);

				Ok(get_result_weight(result).map(|w| {
					T::WeightInfo::propose_execute(
						proposal_len as u32, // B
						members.len() as u32, // M
					).saturating_add(w) // P1
				}).into())
			} else {
				let active_proposals =
					<Proposals<T, I>>::try_mutate(room_id, |proposals| -> Result<usize, DispatchError> {
						proposals.push(proposal_hash);
						ensure!(
							proposals.len() <= T::MaxProposals::get() as usize,
							Error::<T, I>::TooManyProposals
						);
						Ok(proposals.len())
					})?;
				let index = Self::proposal_count(room_id);
				<ProposalCount<I>>::mutate(room_id, |i| *i += 1);
				<ProposalOf<T, I>>::insert(room_id, proposal_hash, *proposal);
				let end = system::Pallet::<T>::block_number() + T::MotionDuration::get();
				let votes = RoomCollectiveVotes { index, reason: reason, threshold, ayes: vec![who.clone()], nays: vec![], end };
				<Voting<T, I>>::insert(room_id, proposal_hash, votes);

				Self::deposit_event(RawEvent::Proposed(who, index, proposal_hash, threshold));

				Ok(Some(T::WeightInfo::propose_proposed(
					proposal_len as u32, // B
					members.len() as u32, // M
					active_proposals as u32, // P2
				)).into())
			}
		}


		#[weight = (
			T::WeightInfo::vote(T::MaxMembers::get()),
			DispatchClass::Operational
		)]
		fn vote(origin,
			room_id: RoomIndex,
			proposal: T::Hash,
			#[compact] index: ProposalIndex,
			approve: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let members = T::ListenHandler::get_room_council(room_id)?;
			ensure!(members.contains(&who), Error::<T, I>::NotMember);

			let mut voting = Self::voting(room_id, &proposal).ok_or(Error::<T, I>::ProposalMissing)?;

			ensure!(voting.index == index, Error::<T, I>::WrongIndex);

			let position_yes = voting.ayes.iter().position(|a| a == &who);
			let position_no = voting.nays.iter().position(|a| a == &who);

			// Detects first vote of the member in the motion
			let is_account_voting_first_time = position_yes.is_none() && position_no.is_none();

			if approve {
				if position_yes.is_none() {
					voting.ayes.push(who.clone());
				} else {
					Err(Error::<T, I>::DuplicateVote)?
				}
				if let Some(pos) = position_no {
					voting.nays.swap_remove(pos);
				}
			} else {
				if position_no.is_none() {
					voting.nays.push(who.clone());
				} else {
					Err(Error::<T, I>::DuplicateVote)?
				}
				if let Some(pos) = position_yes {
					voting.ayes.swap_remove(pos);
				}
			}

			let yes_votes = voting.ayes.len() as MemberCount;
			let no_votes = voting.nays.len() as MemberCount;
			Self::deposit_event(RawEvent::Voted(who, proposal, approve, yes_votes, no_votes));

			Voting::<T, I>::insert(room_id, &proposal, voting);

			if is_account_voting_first_time {
				Ok((
					Some(T::WeightInfo::vote(members.len() as u32)),
					Pays::No,
				).into())
			} else {
				Ok((
					Some(T::WeightInfo::vote(members.len() as u32)),
					Pays::Yes,
				).into())
			}
		}


		#[weight = (
			{
				let b = *length_bound;
				let m = T::MaxMembers::get();
				let p1 = *proposal_weight_bound;
				let p2 = T::MaxProposals::get();
				T::WeightInfo::close_early_approved(b, m, p2)
					.max(T::WeightInfo::close_early_disapproved(m, p2))
					.max(T::WeightInfo::close_approved(b, m, p2))
					.max(T::WeightInfo::close_disapproved(m, p2))
					.saturating_add(p1)
			},
			DispatchClass::Operational
		)]


		fn close(origin,
			room_id: RoomIndex,
			proposal_hash: T::Hash,
			#[compact] index: ProposalIndex,
			#[compact] proposal_weight_bound: Weight,
			#[compact] length_bound: u32
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			let voting = Self::voting(room_id, &proposal_hash).ok_or(Error::<T, I>::ProposalMissing)?;
			ensure!(voting.index == index, Error::<T, I>::WrongIndex);

			let mut no_votes = voting.nays.len() as MemberCount;
			let mut yes_votes = voting.ayes.len() as MemberCount;
			let seats = T::ListenHandler::get_room_council(room_id)?.len() as MemberCount;

			let approved = yes_votes >= voting.threshold;
			let disapproved = seats.saturating_sub(no_votes) < voting.threshold;
			// Allow (dis-)approving the proposal as soon as there are enough votes.
			if approved {
				let (proposal, len) = Self::validate_and_get_proposal(
					room_id,
					&proposal_hash,
					length_bound,
					proposal_weight_bound,
				)?;
				Self::deposit_event(RawEvent::Closed(proposal_hash, yes_votes, no_votes));
				let (proposal_weight, proposal_count) =
					Self::do_approve_proposal(room_id, seats, voting, proposal_hash, proposal);
				return Ok((
					Some(T::WeightInfo::close_early_approved(len as u32, seats, proposal_count)
					.saturating_add(proposal_weight)),
					Pays::Yes,
				).into());

			} else if disapproved {
				Self::deposit_event(RawEvent::Closed(proposal_hash, yes_votes, no_votes));
				let proposal_count = Self::do_disapprove_proposal(room_id, proposal_hash);
				return Ok((
					Some(T::WeightInfo::close_early_disapproved(seats, proposal_count)),
					Pays::No,
				).into());
			}

			// Only allow actual closing of the proposal after the voting period has ended.
			ensure!(system::Pallet::<T>::block_number() >= voting.end, Error::<T, I>::TooEarly);

			// let prime_vote = Self::prime().map(|who| voting.ayes.iter().any(|a| a == &who));
			let prime_vote = T::ListenHandler::get_prime(room_id)?.map(|who| voting.ayes.iter().any(|a| a == &who));

			// default voting strategy.
			/// fixme ??????????????????
			let default = T::DefaultVote::default_vote(prime_vote, yes_votes, no_votes, seats);

			/// fixme ?????????????????????
			let abstentions = seats - (yes_votes + no_votes);

			match default {
				true => yes_votes += abstentions,
				false => no_votes += abstentions,
			}

			let approved = yes_votes >= voting.threshold;

			if approved {
				let (proposal, len) = Self::validate_and_get_proposal(
					room_id,
					&proposal_hash,
					length_bound,
					proposal_weight_bound,
				)?;
				Self::deposit_event(RawEvent::Closed(proposal_hash, yes_votes, no_votes));
				let (proposal_weight, proposal_count) =
					Self::do_approve_proposal(room_id, seats, voting, proposal_hash, proposal);
				return Ok((
					Some(T::WeightInfo::close_approved(len as u32, seats, proposal_count)
					.saturating_add(proposal_weight)),
					Pays::Yes,
				).into());
			} else {
				Self::deposit_event(RawEvent::Closed(proposal_hash, yes_votes, no_votes));
				let proposal_count = Self::do_disapprove_proposal(room_id, proposal_hash);
				return Ok((
					Some(T::WeightInfo::close_disapproved(seats, proposal_count)),
					Pays::No,
				).into());
			}
		}


		#[weight = T::WeightInfo::disapprove_proposal(T::MaxProposals::get())]
		fn disapprove_proposal(origin, room_id: RoomIndex, proposal_hash: T::Hash) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let proposal_count = Self::do_disapprove_proposal(room_id, proposal_hash);
			Ok(Some(T::WeightInfo::disapprove_proposal(proposal_count)).into())
		}
	}
}

impl<T: Config<I>, I: Instance> Module<T, I> {
	/// Check whether `who` is a member of the collective.
	// pub fn is_member(who: &T::AccountId) -> bool {
	// 	// Note: The dispatchables *do not* use this to check membership so make sure
	// 	// to update those if this is changed.
	// 	Self::members().contains(who)
	// }

	/// Ensure that the right proposal bounds were passed and get the proposal from storage.
	///
	/// Checks the length in storage via `storage::read` which adds an extra `size_of::<u32>() == 4`
	/// to the length.
	fn validate_and_get_proposal(
		room_id: RoomIndex,
		hash: &T::Hash,
		length_bound: u32,
		weight_bound: Weight
	) -> Result<(<T as Config<I>>::Proposal, usize), DispatchError> {
		let key = ProposalOf::<T, I>::hashed_key_for(room_id, hash);
		// read the length of the proposal storage entry directly
		let proposal_len = storage::read(&key, &mut [0; 0], 0)
			.ok_or(Error::<T, I>::ProposalMissing)?;
		ensure!(proposal_len <= length_bound, Error::<T, I>::WrongProposalLength);
		let proposal = ProposalOf::<T, I>::get(room_id, hash).ok_or(Error::<T, I>::ProposalMissing)?;
		let proposal_weight = proposal.get_dispatch_info().weight;
		ensure!(proposal_weight <= weight_bound, Error::<T, I>::WrongProposalWeight);
		Ok((proposal, proposal_len as usize))
	}


	fn do_approve_proposal(
		room_id: RoomIndex,
		seats: MemberCount,
		voting: RoomCollectiveVotes<T::AccountId, T::BlockNumber>,
		proposal_hash: T::Hash,
		proposal: <T as Config<I>>::Proposal,
	) -> (Weight, u32) {
		Self::deposit_event(RawEvent::Approved(proposal_hash));

		let dispatch_weight = proposal.get_dispatch_info().weight;
		let origin = RawOrigin::Members(voting.threshold, seats).into();
		let result = proposal.dispatch(origin);
		Self::deposit_event(
			RawEvent::Executed(proposal_hash, result.map(|_| ()).map_err(|e| e.error))
		);
		// default to the dispatch info weight for safety
		let proposal_weight = get_result_weight(result).unwrap_or(dispatch_weight); // P1

		let proposal_count = Self::remove_proposal(room_id, proposal_hash);
		(proposal_weight, proposal_count)
	}

	fn do_disapprove_proposal(room_id: RoomIndex, proposal_hash: T::Hash) -> u32 {
		// disapproved
		Self::deposit_event(RawEvent::Disapproved(proposal_hash));
		Self::remove_proposal(room_id, proposal_hash)
	}

	// Removes a proposal from the pallet, cleaning up votes and the vector of proposals.
	fn remove_proposal(room_id: RoomIndex, proposal_hash: T::Hash) -> u32 {
		// remove proposal and vote
		ProposalOf::<T, I>::remove(room_id, &proposal_hash);
		Voting::<T, I>::remove(room_id, &proposal_hash);
		let num_proposals = Proposals::<T, I>::mutate(room_id, |proposals| {
			proposals.retain(|h| h != &proposal_hash);
			proposals.len() + 1 // calculate weight based on original length
		});
		num_proposals as u32
	}
}

// impl<T: Config<I>, I: Instance> ChangeMembers<T::AccountId> for Module<T, I> {
// 	/// Update the members of the collective. Votes are updated and the prime is reset.
// 	///
// 	/// NOTE: Does not enforce the expected `MaxMembers` limit on the amount of members, but
// 	///       the weight estimations rely on it to estimate dispatchable weight.
// 	///
// 	/// # <weight>
// 	/// ## Weight
// 	/// - `O(MP + N)`
// 	///   - where `M` old-members-count (governance-bounded)
// 	///   - where `N` new-members-count (governance-bounded)
// 	///   - where `P` proposals-count
// 	/// - DB:
// 	///   - 1 storage read (codec `O(P)`) for reading the proposals
// 	///   - `P` storage mutations for updating the votes (codec `O(M)`)
// 	///   - 1 storage write (codec `O(N)`) for storing the new members
// 	///   - 1 storage write (codec `O(1)`) for deleting the old prime
// 	/// # </weight>
// 	fn change_members_sorted(
// 		_incoming: &[T::AccountId],
// 		outgoing: &[T::AccountId],
// 		new: &[T::AccountId],
// 	) {
// 		if new.len() > T::MaxMembers::get() as usize {
// 			log::error!(
// 				target: "runtime::collective",
// 				"New members count ({}) exceeds maximum amount of members expected ({}).",
// 				new.len(),
// 				T::MaxMembers::get(),
// 			);
// 		}
// 		// remove accounts from all current voting in motions.
// 		let mut outgoing = outgoing.to_vec();
// 		outgoing.sort();
// 		for h in Self::proposals().into_iter() {
// 			<Voting<T, I>>::mutate(h, |v|
// 				if let Some(mut votes) = v.take() {
// 					votes.ayes = votes.ayes.into_iter()
// 						.filter(|i| outgoing.binary_search(i).is_err())
// 						.collect();
// 					votes.nays = votes.nays.into_iter()
// 						.filter(|i| outgoing.binary_search(i).is_err())
// 						.collect();
// 					*v = Some(votes);
// 				}
// 			);
// 		}
// 		Members::<T, I>::put(new);
// 		Prime::<T, I>::kill();
// 	}
//
// 	fn set_prime(prime: Option<T::AccountId>) {
// 		Prime::<T, I>::set(prime);
// 	}
//
// 	fn get_prime() -> Option<T::AccountId> {
// 		Prime::<T, I>::get()
// 	}
// }

// impl<T: Config<I>, I: Instance> InitializeMembers<T::AccountId> for Module<T, I> {
// 	fn initialize_members(members: &[T::AccountId]) {
// 		if !members.is_empty() {
// 			assert!(<Members<T, I>>::get().is_empty(), "Members are already initialized!");
// 			<Members<T, I>>::put(members);
// 		}
// 	}
// }

// /// Ensure that the origin `o` represents at least `n` members. Returns `Ok` or an `Err`
// /// otherwise.
// pub fn ensure_members<OuterOrigin, AccountId, I>(o: OuterOrigin, n: MemberCount)
// 	-> result::Result<MemberCount, &'static str>
// where
// 	OuterOrigin: Into<result::Result<RawOrigin<AccountId, I>, OuterOrigin>>
// {
// 	match o.into() {
// 		Ok(RawOrigin::Members(x, _)) if x >= n => Ok(n),
// 		_ => Err("bad origin: expected to be a threshold number of members"),
// 	}
// }
//
pub struct EnsureMember<AccountId, I=DefaultInstance>(sp_std::marker::PhantomData<(AccountId, I)>);
impl<
	O: Into<Result<RawOrigin<AccountId, I>, O>> + From<RawOrigin<AccountId, I>>,
	AccountId: Default,
	I,
> EnsureOrigin<O> for EnsureMember<AccountId, I> {
	type Success = AccountId;
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Member(id, who) => Ok(who),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> O {
		O::from(RawOrigin::Member(Default::default(), Default::default()))
	}
}

pub struct EnsureMembers<N: U32, AccountId, I=DefaultInstance>(sp_std::marker::PhantomData<(N, AccountId, I)>);
impl<
	O: Into<Result<RawOrigin<AccountId, I>, O>> + From<RawOrigin<AccountId, I>>,
	N: U32,
	AccountId,
	I,
> EnsureOrigin<O> for EnsureMembers<N, AccountId, I> {
	type Success = (MemberCount, MemberCount);
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Members(n, m) if n >= N::VALUE => Ok((n, m)),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> O {
		O::from(RawOrigin::Members(N::VALUE, N::VALUE))
	}
}

pub struct EnsureRoomRoot<T, AccountId=<T as frame_system::Config>::AccountId, I=DefaultInstance>(sp_std::marker::PhantomData<(T, AccountId, I)>);

impl<O: Into<Result<RawOrigin<<T as frame_system::Config>::AccountId, I>, O>> + From<RawOrigin<<T as frame_system::Config>::AccountId, I>>,
	AccountId,
	T: Config<I>,
	I: Instance,

> EnsureOrigin<O> for EnsureRoomRoot<T, AccountId, I> {

	type Success = ();
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Member(room_id, who) if T::ListenHandler::get_root(room_id).is_ok() && T::ListenHandler::get_root(room_id).unwrap() == who => Ok(()),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> O {
		unimplemented!()
	}
}

pub struct EnsureProportionMoreThan<N: U32, D: U32, AccountId, I=DefaultInstance>(
	sp_std::marker::PhantomData<(N, D, AccountId, I)>
);
impl<
	O: Into<Result<RawOrigin<AccountId, I>, O>> + From<RawOrigin<AccountId, I>>,
	N: U32,
	D: U32,
	AccountId,
	I,
> EnsureOrigin<O> for EnsureProportionMoreThan<N, D, AccountId, I> {
	type Success = ();
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Members(n, m) if n * D::VALUE > N::VALUE * m => Ok(()),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> O {
		O::from(RawOrigin::Members(1u32, 0u32))
	}
}

pub struct EnsureProportionAtLeast<N: U32, D: U32, AccountId, I=DefaultInstance>(
	sp_std::marker::PhantomData<(N, D, AccountId, I)>
);
impl<
	O: Into<Result<RawOrigin<AccountId, I>, O>> + From<RawOrigin<AccountId, I>>,
	N: U32,
	D: U32,
	AccountId,
	I,
> EnsureOrigin<O> for EnsureProportionAtLeast<N, D, AccountId, I> {
	type Success = ();
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Members(n, m) if n * D::VALUE >= N::VALUE * m => Ok(()),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> O {
		O::from(RawOrigin::Members(0u32, 0u32))
	}
}

impl<T: Config<I>, I: Instance> CollectiveHandler<u64, DispatchError> for Module<T, I> {
	fn remove_room_collective_info(room_id: u64) -> result::Result<(), DispatchError> {
		<ProposalCount>::remove(room_id);
		<Voting<T, I>>::remove_prefix(room_id);
		<ProposalOf<T, I>>::remove_prefix(room_id);
		<Proposals<T, I>>::remove(room_id);
		Ok(())
	}
}