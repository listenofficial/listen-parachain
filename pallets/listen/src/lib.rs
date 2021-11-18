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

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod primitives;
pub use crate::pallet::*;
use crate::primitives::{
	listen_time, vote, AllProps, Audio, AudioPrice, CreateCost, DisbandTime, DisbandVote,
	GroupInfo, GroupMaxMembers, ListenVote, PersonInfo, PropsPrice, RedPacket, RemoveTime,
	RewardStatus, RoomId, RoomRewardInfo, SessionIndex,
};
use codec::{Decode, Encode};
pub use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{
		BalanceStatus as Status, Currency, EnsureOrigin,
		ExistenceRequirement::{AllowDeath, KeepAlive},
		Get, OnUnbalanced, ReservableCurrency, WithdrawReasons,
	},
	weights::Weight,
	Blake2_256, IterableStorageDoubleMap, IterableStorageMap, PalletId,
};
use frame_system::{self as system, ensure_root, ensure_signed};
use listen_primitives::{
	constants::{currency::*, time::*},
	traits::{CollectiveHandler, ListenHandler, RoomTreasuryHandler},
	*,
};
use listen_time::*;
use orml_tokens::{self, BalanceLock};
use orml_traits::MultiCurrency;
use pallet_multisig;
use pallet_timestamp as timestamp;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		AccountIdConversion, CheckedAdd, CheckedDiv, CheckedMul, Saturating, StaticLookup, Zero,
	},
	DispatchError, DispatchResult, Percent, RuntimeDebug, SaturatedConversion,
};
use sp_std::{
	cmp,
	collections::{btree_map::BTreeMap, btree_set::BTreeSet},
	convert::{TryFrom, TryInto},
	prelude::*,
	result,
};
use vote::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::{
			Blake2_128Concat, IsType, OptionQuery, StorageDoubleMap, StorageMap, StorageValue,
			ValueQuery,
		},
		traits::Hooks,
	};
	use frame_system::pallet_prelude::*;

	pub(crate) type MultiBalanceOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	pub(crate) type CurrencyIdOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
		<T as frame_system::Config>::AccountId,
	>>::CurrencyId;
	type BalanceOf<T> =
		<<T as Config>::NativeCurrency as Currency<<T as system::Config>::AccountId>>::Balance;
	type PositiveImbalanceOf<T> = <<T as Config>::NativeCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::PositiveImbalance;
	type NegativeImbalanceOf<T> = <<T as Config>::NativeCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	#[pallet::config]
	#[pallet::disable_frame_system_supertrait_check]
	pub trait Config: system::Config + timestamp::Config + pallet_multisig::Config {
		type Event: From<Event<Self>>
			+ Into<<Self as system::Config>::Event>
			+ IsType<<Self as frame_system::Config>::Event>;
		type Create: OnUnbalanced<PositiveImbalanceOf<Self>>;
		type NativeCurrency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
		type MultiCurrency: MultiCurrency<Self::AccountId>;
		type ProposalRejection: OnUnbalanced<NegativeImbalanceOf<Self>>;
		type CollectiveHandler: CollectiveHandler<u64, Self::BlockNumber, DispatchError>;
		type RoomRootOrigin: EnsureOrigin<Self::Origin>;
		type RoomRootOrHalfCouncilOrigin: EnsureOrigin<Self::Origin>;
		type RoomRootOrHalfRoomCouncilOrSomeRoomCouncilOrigin: EnsureOrigin<Self::Origin>;
		type HalfRoomCouncilOrigin: EnsureOrigin<Self::Origin>;
		type RoomTreasuryHandler: RoomTreasuryHandler<u64>;
		#[pallet::constant]
		type VoteExpire: Get<Self::BlockNumber>;
		#[pallet::constant]
		type RedPacketMinAmount: Get<BalanceOf<Self>>;
		#[pallet::constant]
		type RedPackExpire: Get<Self::BlockNumber>;
		#[pallet::constant]
		type RewardDuration: Get<Self::BlockNumber>;
		#[pallet::constant]
		type ManagerProportion: Get<Percent>;
		#[pallet::constant]
		type RoomProportion: Get<Percent>;
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		/// The amount of airdrops each person gets
		#[pallet::constant]
		type AirDropAmount: Get<BalanceOf<Self>>;
		/// LT currency id in the tokens module
		#[pallet::constant]
		type GetNativeCurrencyId: Get<CurrencyIdOf<Self>>;
		/// Duration of group protection
		#[pallet::constant]
		type ProtectedDuration: Get<Self::BlockNumber>;
		/// The maximum number of members of a group council.
		#[pallet::constant]
		type CouncilMaxNumber: Get<u32>;
		/// Delay duration of group disbanding
		#[pallet::constant]
		type DelayDisbandDuration: Get<Self::BlockNumber>;
	}

	/// People who have already received airdrops
	#[pallet::storage]
	#[pallet::getter(fn alreadly_air_drop_list)]
	pub type AlreadyAirDropList<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn group_id)]
	pub type GroupId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// The cost of creating a group
	#[pallet::storage]
	#[pallet::getter(fn create_cost)]
	pub type CreatePayment<T: Config> = StorageValue<_, CreateCost, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn red_packet_id)]
	pub type RedPacketId<T: Config> = StorageValue<_, u128, ValueQuery>;

	/// All groups that have been created
	#[pallet::storage]
	#[pallet::getter(fn all_room)]
	pub type AllRoom<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		u64,
		GroupInfo<
			T::AccountId,
			BalanceOf<T>,
			AllProps,
			Audio,
			T::BlockNumber,
			GroupMaxMembers,
			DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
			T::Moment,
		>,
	>;

	/// Everyone in the room (People who have not yet claimed their reward).
	#[pallet::storage]
	#[pallet::getter(fn listeners_of_room)]
	pub type ListenersOfRoom<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, BTreeSet<T::AccountId>, ValueQuery>;

	/// Specific information about each person's purchase
	#[pallet::storage]
	#[pallet::getter(fn all_listeners)]
	pub type AllListeners<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		PersonInfo<AllProps, Audio, BalanceOf<T>, RewardStatus>,
		ValueQuery,
	>;

	/// Disbanded rooms
	///
	/// (如果要查询投票是否还在进行 通过room_info来查，不是在这个存储里查，并且配合投票过期时间)
	#[pallet::storage]
	#[pallet::getter(fn info_of_disband_room)]
	pub type InfoOfDisbandedRoom<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		SessionIndex,
		Blake2_128Concat,
		u64,
		RoomRewardInfo<BalanceOf<T>>,
		ValueQuery,
	>;

	/// All the red packets in the group
	#[pallet::storage]
	#[pallet::getter(fn red_packets_of_room)]
	pub type RedPacketOfRoom<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		u64,
		Blake2_128Concat,
		u128,
		RedPacket<
			T::AccountId,
			BTreeSet<T::AccountId>,
			BalanceOf<T>,
			T::BlockNumber,
			CurrencyIdOf<T>,
		>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn multisig)]
	pub type Multisig<T: Config> = StorageValue<_, (Vec<T::AccountId>, u16, T::AccountId)>;

	#[pallet::storage]
	#[pallet::getter(fn server_id)]
	pub type ServerId<T: Config> = StorageValue<_, T::AccountId>;

	/// Vote on dissolution has been passed by the group
	///
	/// (如果要查询投票是否还在进行 通过room_info来查，并且配合投票过期时间)
	#[pallet::storage]
	#[pallet::getter(fn disband_rooms)]
	pub type DisbandingRooms<T: Config> =
		StorageValue<_, BTreeMap<u64, T::BlockNumber>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn last_session_index)]
	pub type LastSessionIndex<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn session_index)]
	pub type NowSessionIndex<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::type_value]
	pub fn DepthOnEmpty() -> u32 {
		20u32
	}
	/// The maximum number of sessions a user can get reward.
	#[pallet::storage]
	#[pallet::getter(fn depth)]
	pub type Depth<T: Config> = StorageValue<_, u32, ValueQuery, DepthOnEmpty>;

	#[pallet::type_value]
	pub fn RemoveIntervalOnEmpty<T: Config>() -> RemoveTime<T::BlockNumber> {
		RemoveTime {
			Ten: T::BlockNumber::from(remove::Ten),
			Hundred: T::BlockNumber::from(remove::Hundred),
			FiveHundred: T::BlockNumber::from(remove::FiveHundred),
			TenThousand: T::BlockNumber::from(remove::TenThousand),
			NoLimit: T::BlockNumber::from(remove::NoLimit),
		}
	}
	#[pallet::storage]
	#[pallet::getter(fn kick_time_limit)]
	pub type RemoveInterval<T: Config> =
		StorageValue<_, RemoveTime<T::BlockNumber>, ValueQuery, RemoveIntervalOnEmpty<T>>;

	#[pallet::type_value]
	pub fn DisbandIntervalOnEmpty<T: Config>() -> DisbandTime<T::BlockNumber> {
		DisbandTime {
			Ten: T::BlockNumber::from(disband::Ten),
			Hundred: T::BlockNumber::from(disband::Hundred),
			FiveHundred: T::BlockNumber::from(disband::FiveHundred),
			TenThousand: T::BlockNumber::from(disband::TenThousand),
			NoLimit: T::BlockNumber::from(disband::NoLimit),
		}
	}
	#[pallet::storage]
	#[pallet::getter(fn disband_time_limit)]
	pub type DisbandInterval<T: Config> =
		StorageValue<_, DisbandTime<T::BlockNumber>, ValueQuery, DisbandIntervalOnEmpty<T>>;

	#[pallet::type_value]
	pub fn PropsPaymentOnEmpty<T: Config>() -> PropsPrice<BalanceOf<T>> {
		PropsPrice {
			picture: <BalanceOf<T> as TryFrom<Balance>>::try_from(Percent::from_percent(3) * UNIT)
				.ok()
				.unwrap(),
			text: <BalanceOf<T> as TryFrom<Balance>>::try_from(Percent::from_percent(1) * UNIT)
				.ok()
				.unwrap(),
			video: <BalanceOf<T> as TryFrom<Balance>>::try_from(Percent::from_percent(3) * UNIT)
				.ok()
				.unwrap(),
		}
	}
	#[pallet::storage]
	#[pallet::getter(fn props_payment)]
	pub type PropsPayment<T: Config> =
		StorageValue<_, PropsPrice<BalanceOf<T>>, ValueQuery, PropsPaymentOnEmpty<T>>;

	#[pallet::type_value]
	pub fn AudioPaymentOnEmpty<T: Config>() -> AudioPrice<BalanceOf<T>> {
		AudioPrice {
			ten_seconds: <BalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(1) * UNIT,
			)
			.ok()
			.unwrap(),
			thirty_seconds: <BalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(2) * UNIT,
			)
			.ok()
			.unwrap(),
			minutes: <BalanceOf<T> as TryFrom<Balance>>::try_from(Percent::from_percent(2) * UNIT)
				.ok()
				.unwrap(),
		}
	}
	#[pallet::storage]
	#[pallet::getter(fn audio_payment)]
	pub type AudioPayment<T: Config> =
		StorageValue<_, AudioPrice<BalanceOf<T>>, ValueQuery, AudioPaymentOnEmpty<T>>;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// #[pallet::weight(10_000)]
		// pub fn test(origin: OriginFor<T>) -> DispatchResult {
		// 	let _ = ensure_signed(origin)?;
		// 	Self::deposit_event(Event::ListenTest);
		// 	Ok(())
		// }
		/// Set a multi-sign account
		///
		/// The Origin must be LISTEN official service account.
		#[pallet::weight(10_000)]
		pub fn set_multisig(
			origin: OriginFor<T>,
			members: Vec<<T::Lookup as StaticLookup>::Source>,
			threshould: u16,
		) -> DispatchResult {
			let server_id = ensure_signed(origin)?;

			let len = members.len();
			ensure!(
				len > 0 && threshould > 0u16 && threshould <= len as u16,
				Error::<T>::ThreshouldLenErr
			);
			ensure!(<ServerId<T>>::get().is_some(), Error::<T>::ServerIdNotExists);
			ensure!(<ServerId<T>>::get().unwrap() == server_id.clone(), Error::<T>::NotServerId);

			let members = Self::sort_account_id(Self::check_accounts(members)?)?;
			let multisig_id =
				<pallet_multisig::Module<T>>::multi_account_id(&members, threshould.clone());
			<Multisig<T>>::put((members, threshould, multisig_id));

			Self::deposit_event(Event::SetMultisig);
			Ok(())
		}

		/// Set LISTEN service account.
		///
		/// The Origin must be Root
		#[pallet::weight(10_000)]
		pub fn set_server_id(
			origin: OriginFor<T>,
			server_id: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			ensure_root(origin)?;

			let server_id = T::Lookup::lookup(server_id)?;
			<ServerId<T>>::put(server_id.clone());

			Self::deposit_event(Event::SetServerId(server_id));
			Ok(())
		}

		/// User gets airdrop.
		///
		/// The Origin can be everyone, but your account balances should be zero.
		#[pallet::weight(10_000)]
		pub fn air_drop(
			origin: OriginFor<T>,
			members: Vec<<T::Lookup as StaticLookup>::Source>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let members = Self::check_accounts(members)?;
			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigIdIsNone)?;
			ensure!(who.clone() == multisig_id.clone(), Error::<T>::NotMultisigId);

			for user in members.iter() {
				if <AlreadyAirDropList<T>>::get().contains(&user) {
					continue
				}
				/// the account balances should be zero.
				if T::NativeCurrency::total_balance(&user) != Zero::zero() {
					continue
				}
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
					&user,
					T::AirDropAmount::get(),
				));
				<AlreadyAirDropList<T>>::mutate(|h| h.insert(user.clone()));
				<system::Module<T>>::inc_ref(&user);
			}
			Self::deposit_event(Event::AirDroped(who));

			Ok(())
		}

		/// The user creates a room.
		///
		/// Everyone can do it, and he(she) will be room manager.
		#[pallet::weight(10_000)]
		pub fn create_room(
			origin: OriginFor<T>,
			max_members: GroupMaxMembers,
			group_type: Vec<u8>,
			join_cost: BalanceOf<T>,
			is_private: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let create_cost = Self::create_cost();
			let create_payment: Balance = match max_members.clone() {
				GroupMaxMembers::Ten => create_cost.Ten,
				GroupMaxMembers::Hundred => create_cost.Hundred,
				GroupMaxMembers::FiveHundred => create_cost.FiveHundred,
				GroupMaxMembers::TenThousand => create_cost.TenThousand,
				GroupMaxMembers::NoLimit => create_cost.NoLimit,
				_ => return Err(Error::<T>::UnknownRoomType)?,
			};
			let create_payment = <BalanceOf<T> as TryFrom<Balance>>::try_from(create_payment)
				.map_err(|_| Error::<T>::NumberCanNotConvert)?;

			// let reasons = WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE;
			// let free_balance = T::NativeCurrency::free_balance(&who);
			// let new_balance = free_balance.saturating_sub(create_payment);
			// T::NativeCurrency::ensure_can_withdraw(&who, <BalanceOf<T>>::from(10u32), reasons, new_balance)?;

			/// the create cost transfers to the treasury
			let to = Self::treasury_id();
			T::NativeCurrency::transfer(&who, &to, create_payment.clone(), KeepAlive)?;
			let group_id = <GroupId<T>>::get();

			let group_info = GroupInfo {
				group_id,
				create_payment,
				last_block_of_get_the_reward: Self::now(),
				group_manager: who.clone(),
				prime: None,
				max_members,
				group_type,
				join_cost,
				props: AllProps::default(),
				audio: Audio::default(),
				total_balances: <BalanceOf<T>>::from(0u32),
				group_manager_balances: <BalanceOf<T>>::from(0u32),
				now_members_number: 1u32,
				last_remove_someone_block: T::BlockNumber::default(),
				disband_vote_end_block: T::BlockNumber::default(),
				disband_vote: DisbandVote::default(),
				create_time: <timestamp::Module<T>>::get(),
				create_block: Self::now(),
				consume: vec![],
				council: vec![],
				black_list: vec![],
				is_private,
			};

			<AllRoom<T>>::insert(group_id, group_info);
			Self::add_listener_info(who.clone(), group_id);
			<GroupId<T>>::mutate(|h| *h += 1);
			Self::deposit_event(Event::CreatedRoom(who, group_id));

			Ok(())
		}

		/// The room manager get his reward.
		///
		/// You should have already created the room.
		/// Receive rewards for a fixed period of time
		#[pallet::weight(10_000)]
		pub fn manager_get_reward(origin: OriginFor<T>, group_id: u64) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			let mut room_info = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let who = room_info.group_manager.clone();
			let last_block = room_info.last_block_of_get_the_reward.clone();
			let now = Self::now();
			let time = now.saturating_sub(last_block);
			let mut duration_num =
				time.checked_div(&T::RewardDuration::get()).ok_or(Error::<T>::DivByZero)?;
			let real_this_block =
				last_block.saturating_add(duration_num * T::RewardDuration::get());

			if duration_num.is_zero() {
				return Err(Error::<T>::NotRewardTime)?
			} else {
				if duration_num > 5u32.saturated_into::<T::BlockNumber>() {
					duration_num = 5u32.saturated_into::<T::BlockNumber>();
				}
				let duration_num = duration_num.saturated_into::<u32>();
				let consume_total_amount = Self::get_room_consume_amount(room_info.clone());
				let manager_proportion_amount = T::ManagerProportion::get() *
					consume_total_amount * duration_num
					.saturated_into::<BalanceOf<T>>();
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
					&who,
					manager_proportion_amount,
				));
				room_info.total_balances =
					room_info.total_balances.saturating_sub(manager_proportion_amount);
				let room_add = T::RoomProportion::get() *
					consume_total_amount * duration_num.saturated_into::<BalanceOf<T>>();
				room_info.total_balances =
					room_info.total_balances.clone().saturating_add(room_add);

				room_info.last_block_of_get_the_reward = real_this_block;
				<AllRoom<T>>::insert(group_id, room_info);

				Self::deposit_event(Event::ManagerGetReward(
					who,
					manager_proportion_amount,
					room_add,
				));

				Ok(())
			}
		}

		/// The room manager modify the cost of group entry.
		///
		/// The Origin must be RoomManager.
		#[pallet::weight(10_000)]
		pub fn update_join_cost(
			origin: OriginFor<T>,
			group_id: u64,
			join_cost: BalanceOf<T>,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			let mut room_info = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			ensure!(
				room_info.join_cost.clone() != join_cost.clone(),
				Error::<T>::AmountShouldDifferent
			);
			room_info.join_cost = join_cost.clone();
			<AllRoom<T>>::insert(group_id, room_info);
			Self::deposit_event(Event::JoinCostChanged(group_id, join_cost));
			Ok(())
		}

		/// The user joins the room.
		///
		/// If invitee is None, you go into the room by yourself. Otherwise, you invite people in.
		/// You can not invite yourself.
		#[pallet::weight(10_000)]
		pub fn into_room(
			origin: OriginFor<T>,
			group_id: u64,
			invitee: Option<<T::Lookup as StaticLookup>::Source>,
		) -> DispatchResult {
			let inviter = ensure_signed(origin)?;

			let invitee = match invitee {
				None => None,
				Some(x) => Some(T::Lookup::lookup(x)?),
			};

			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			let room_info = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;

			if room_info.is_private.clone() {
				ensure!(
					invitee.is_some() && inviter.clone() == room_info.group_manager.clone(),
					Error::<T>::PrivateRoom
				);
			}

			let mut invitee = invitee.clone();

			if invitee.is_some() {
				ensure!(invitee.clone().unwrap() != inviter.clone(), Error::<T>::ShouldNotYourself);
				/// the inviter must be in this room.
				ensure!(Self::is_in_room(group_id, inviter.clone())?, Error::<T>::NotInRoom);
			}

			/// people who into the room must be not in blacklist.
			let black_list = room_info.black_list;
			let man = invitee.clone().unwrap_or_else(|| inviter.clone());
			if let Some(pos) = black_list.iter().position(|h| h == &man) {
				return Err(Error::<T>::InBlackList)?
			};

			ensure!(
				room_info.max_members.clone().into_u32()? >=
					room_info.now_members_number.clone() + 1,
				Error::<T>::MembersNumberToMax
			);
			ensure!(!(Self::is_in_room(group_id, invitee.clone().unwrap())?), Error::<T>::InRoom);

			Self::join_do(inviter.clone(), invitee.clone(), group_id)?;

			Self::deposit_event(Event::IntoRoom(invitee, inviter, group_id));
			Ok(())
		}

		/// Set the privacy properties of the room
		///
		/// The Origin must be room manager.
		#[pallet::weight(10_000)]
		pub fn set_room_privacy(
			origin: OriginFor<T>,
			room_id: u64,
			is_private: bool,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			ensure!(!Self::is_in_disbanding(room_id)?, Error::<T>::Disbanding);
			let mut room = <AllRoom<T>>::get(room_id).ok_or(Error::<T>::RoomNotExists)?;
			ensure!(room.is_private.clone() != is_private, Error::<T>::PrivacyNotChange);
			room.is_private = is_private;
			<AllRoom<T>>::insert(room_id, room);

			Self::deposit_event(Event::SetRoomPrivacy(room_id, is_private));
			Ok(())
		}

		/// Set the maximum number of people in the room
		///
		/// The Origin must be room manager.
		#[pallet::weight(10_000)]
		pub fn set_max_number_of_room_members(
			origin: OriginFor<T>,
			group_id: u64,
			new_max: GroupMaxMembers,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			let mut room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let manager = room.group_manager.clone();
			let now_number = room.now_members_number;
			let now_max = room.max_members;
			ensure!(now_max != new_max, Error::<T>::RoomMaxNotDiff);
			let now_max_number = now_max.into_u32().map_err(|_| Error::<T>::MaxMembersUnknown)?;
			let new_max_number = new_max.into_u32().map_err(|_| Error::<T>::MaxMembersUnknown)?;
			ensure!(now_number <= new_max_number, Error::<T>::RoomMembersToMax);

			let old_amount = match now_max {
				GroupMaxMembers::Ten => CreatePayment::<T>::get().Ten,
				GroupMaxMembers::Hundred => CreatePayment::<T>::get().Hundred,
				GroupMaxMembers::FiveHundred => CreatePayment::<T>::get().FiveHundred,
				GroupMaxMembers::TenThousand => CreatePayment::<T>::get().TenThousand,
				GroupMaxMembers::NoLimit => CreatePayment::<T>::get().NoLimit,
				_ => return Err(Error::<T>::UnknownRoomType)?,
			};

			let new_amount = match new_max {
				GroupMaxMembers::Ten => CreatePayment::<T>::get().Ten,
				GroupMaxMembers::Hundred => CreatePayment::<T>::get().Hundred,
				GroupMaxMembers::FiveHundred => CreatePayment::<T>::get().FiveHundred,
				GroupMaxMembers::TenThousand => CreatePayment::<T>::get().TenThousand,
				GroupMaxMembers::NoLimit => CreatePayment::<T>::get().NoLimit,
				_ => return Err(Error::<T>::UnknownRoomType)?,
			};

			let add_amount = new_amount.saturating_sub(old_amount).saturated_into::<BalanceOf<T>>();
			if add_amount != Zero::zero() {
				/// transfers amount to the treasury.
				let to = Self::treasury_id();
				T::NativeCurrency::transfer(&manager, &to, add_amount.clone(), KeepAlive)?;
			}

			room.max_members = new_max;
			room.create_payment = new_amount.saturated_into::<BalanceOf<T>>();
			<AllRoom<T>>::insert(group_id, room);

			Self::deposit_event(Event::SetMaxNumberOfRoomMembers(manager, new_max_number));
			Ok(())
		}

		/// Users buy props in the room.
		#[pallet::weight(10_000)]
		pub fn buy_props_in_room(
			origin: OriginFor<T>,
			group_id: u64,
			props: AllProps,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);
			let mut dollars = <BalanceOf<T>>::from(0u32);
			ensure!(
				props.picture != 0u32 || props.text != 0u32 || props.video != 0u32,
				Error::<T>::BuyNothing
			);
			let props_cost = <PropsPayment<T>>::get();

			if props.picture > 0u32 {
				dollars = props_cost
					.picture
					.checked_mul(&<BalanceOf<T>>::from(props.picture))
					.ok_or(Error::<T>::Overflow)?;
			}

			if props.text > 0u32 {
				dollars = dollars
					.checked_add(
						&props_cost
							.text
							.checked_mul(&<BalanceOf<T>>::from(props.text))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

			if props.video > 0u32 {
				dollars = dollars
					.checked_add(
						&props_cost
							.video
							.checked_mul(&<BalanceOf<T>>::from(props.video))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

			let mut room = <AllRoom<T>>::get(group_id).unwrap();
			let mut person = <AllListeners<T>>::get(who.clone());

			room.props.picture =
				room.props.picture.checked_add(props.picture).ok_or(Error::<T>::Overflow)?;
			room.props.text =
				room.props.text.checked_add(props.text).ok_or(Error::<T>::Overflow)?;
			room.props.video =
				room.props.video.checked_add(props.video).ok_or(Error::<T>::Overflow)?;
			room.total_balances =
				room.total_balances.checked_add(&dollars.clone()).ok_or(Error::<T>::Overflow)?;

			person.props.picture =
				person.props.picture.checked_add(props.picture).ok_or(Error::<T>::Overflow)?;
			person.props.text =
				person.props.text.checked_add(props.text).ok_or(Error::<T>::Overflow)?;
			person.props.video =
				person.props.video.checked_add(props.video).ok_or(Error::<T>::Overflow)?;
			person.cost = person.cost.checked_add(&dollars.clone()).ok_or(Error::<T>::Overflow)?;

			T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(
				&who,
				dollars.clone(),
				WithdrawReasons::TRANSFER.into(),
				KeepAlive,
			)?);
			Self::update_user_consume(who.clone(), room, dollars);
			<AllListeners<T>>::insert(who.clone(), person);

			Self::deposit_event(Event::BuyProps(who));
			Ok(())
		}

		/// Users buy audio in the room.
		#[pallet::weight(10_000)]
		pub fn buy_audio_in_room(
			origin: OriginFor<T>,
			group_id: u64,
			audio: Audio,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);
			let mut dollars = <BalanceOf<T>>::from(0u32);
			ensure!(
				audio.ten_seconds != 0u32 || audio.thirty_seconds != 0u32 || audio.minutes != 0u32,
				Error::<T>::BuyNothing
			);
			let audio_cost = <AudioPayment<T>>::get();

			if audio.ten_seconds > 0u32 {
				dollars = audio_cost
					.ten_seconds
					.checked_mul(&<BalanceOf<T>>::from(audio.ten_seconds))
					.ok_or(Error::<T>::Overflow)?;
			}

			if audio.thirty_seconds > 0u32 {
				dollars = dollars
					.checked_add(
						&audio_cost
							.thirty_seconds
							.checked_mul(&<BalanceOf<T>>::from(audio.thirty_seconds))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

			if audio.minutes > 0u32 {
				dollars = dollars
					.checked_add(
						&audio_cost
							.minutes
							.checked_mul(&<BalanceOf<T>>::from(audio.minutes))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

			let mut room = <AllRoom<T>>::get(group_id).unwrap();
			let mut person = <AllListeners<T>>::get(who.clone());

			room.audio.ten_seconds = room
				.audio
				.ten_seconds
				.checked_add(audio.ten_seconds)
				.ok_or(Error::<T>::Overflow)?;
			room.audio.thirty_seconds = room
				.audio
				.thirty_seconds
				.checked_add(audio.thirty_seconds)
				.ok_or(Error::<T>::Overflow)?;
			room.audio.minutes =
				room.audio.minutes.checked_add(audio.minutes).ok_or(Error::<T>::Overflow)?;
			room.total_balances =
				room.total_balances.checked_add(&dollars.clone()).ok_or(Error::<T>::Overflow)?;

			person.audio.ten_seconds = person
				.audio
				.ten_seconds
				.checked_add(audio.ten_seconds)
				.ok_or(Error::<T>::Overflow)?;
			person.audio.thirty_seconds = person
				.audio
				.thirty_seconds
				.checked_add(audio.thirty_seconds)
				.ok_or(Error::<T>::Overflow)?;
			person.audio.minutes =
				person.audio.minutes.checked_add(audio.minutes).ok_or(Error::<T>::Overflow)?;
			person.cost = person.cost.checked_add(&dollars.clone()).ok_or(Error::<T>::Overflow)?;

			T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(
				&who,
				dollars.clone(),
				WithdrawReasons::TRANSFER.into(),
				KeepAlive,
			)?);
			<AllListeners<T>>::insert(who.clone(), person);
			Self::update_user_consume(who.clone(), room, dollars);

			Self::deposit_event(Event::BuyAudio(who));
			Ok(())
		}

		/// Set the cost of creating the room
		///
		/// The Origin must be Root.
		#[pallet::weight(10_000)]
		pub fn set_create_cost(
			origin: OriginFor<T>,
			max_members: GroupMaxMembers,
			amount: Balance,
		) -> DispatchResult {
			ensure_root(origin)?;

			match max_members {
				GroupMaxMembers::Ten => CreatePayment::<T>::mutate(|h| h.Ten = amount),
				GroupMaxMembers::Hundred => CreatePayment::<T>::mutate(|h| h.Hundred = amount),
				GroupMaxMembers::FiveHundred =>
					CreatePayment::<T>::mutate(|h| h.FiveHundred = amount),
				GroupMaxMembers::TenThousand =>
					CreatePayment::<T>::mutate(|h| h.TenThousand = amount),
				GroupMaxMembers::NoLimit => CreatePayment::<T>::mutate(|h| h.NoLimit = amount),
				_ => return Err(Error::<T>::UnknownRoomType)?,
			}

			Self::deposit_event(Event::SetCreateCost);
			Ok(())
		}

		/// Remove someone from a blacklist
		///
		/// The Origin must be RoomCouncil or RoomManager.
		#[pallet::weight(10_000)]
		pub fn remove_someone_from_blacklist(
			origin: OriginFor<T>,
			group_id: u64,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			T::RoomRootOrHalfCouncilOrigin::try_origin(origin)
				.map_err(|_| Error::<T>::BadOrigin)?;

			let who = T::Lookup::lookup(who)?;
			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			let mut room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let black_list = room.black_list.clone();

			if let Some(pos) = black_list.iter().position(|h| h == &who) {
				room.black_list.remove(pos);
			} else {
				return Err(Error::<T>::NotInBlackList)?
			}

			<AllRoom<T>>::insert(group_id, room);

			Self::deposit_event(Event::RemoveSomeoneFromBlackList(who, group_id));
			Ok(())
		}

		/// Remove someone from the room.
		///
		/// The Origin must be RoomCouncil or RoomManager.
		#[pallet::weight(10_000)]
		pub fn remove_someone(
			origin: OriginFor<T>,
			group_id: u64,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			T::RoomRootOrHalfRoomCouncilOrSomeRoomCouncilOrigin::try_origin(origin.clone())
				.map_err(|_| Error::<T>::BadOrigin)?;

			let who = T::Lookup::lookup(who)?;
			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			let mut room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			ensure!(room.group_manager != who.clone(), Error::<T>::RoomManager);
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);
			let now = Self::now();

			if T::RoomRootOrigin::try_origin(origin).is_ok() {
				if room.last_remove_someone_block > T::BlockNumber::from(0u32) {
					let until = now.saturating_sub(room.last_remove_someone_block);
					match room.max_members {
						GroupMaxMembers::Ten =>
							if until <= <RemoveInterval<T>>::get().Ten {
								return Err(Error::<T>::IsNotRemoveTime)?
							},
						GroupMaxMembers::Hundred =>
							if until <= <RemoveInterval<T>>::get().Hundred {
								return Err(Error::<T>::IsNotRemoveTime)?
							},
						GroupMaxMembers::FiveHundred => {
							if until <= <RemoveInterval<T>>::get().FiveHundred {
								return Err(Error::<T>::IsNotRemoveTime)?
							}
						},
						GroupMaxMembers::TenThousand => {
							if until <= <RemoveInterval<T>>::get().TenThousand {
								return Err(Error::<T>::IsNotRemoveTime)?
							}
						},
						GroupMaxMembers::NoLimit =>
							if until <= <RemoveInterval<T>>::get().NoLimit {
								return Err(Error::<T>::IsNotRemoveTime)?
							},
					}
				}
				room.last_remove_someone_block = now;
			}

			room.black_list.push(who.clone());
			let room = Self::remove_someone_in_room(who.clone(), room);
			<AllRoom<T>>::insert(group_id, room);

			Self::deposit_event(Event::Kicked(who.clone(), group_id));
			Ok(())
		}

		/// Request dismissal of the room
		#[pallet::weight(10_000)]
		pub fn ask_for_disband_room(origin: OriginFor<T>, group_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!Self::is_can_disband(group_id)?, Error::<T>::Disbanding);
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);
			let mut room = <AllRoom<T>>::get(group_id).unwrap();
			let create_block = room.create_block;
			let now = Self::now();
			ensure!(
				now - create_block > T::ProtectedDuration::get(),
				Error::<T>::InProtectedDuration
			);

			if room.disband_vote_end_block > T::BlockNumber::from(0u32) {
				let until = now.clone().saturating_sub(room.disband_vote_end_block);
				match room.max_members {
					GroupMaxMembers::Ten =>
						if until <= <DisbandInterval<T>>::get().Ten {
							return Err(Error::<T>::IsNotAskForDisbandTime)?
						},
					GroupMaxMembers::Hundred =>
						if until <= <DisbandInterval<T>>::get().Hundred {
							return Err(Error::<T>::IsNotAskForDisbandTime)?
						},
					GroupMaxMembers::FiveHundred => {
						if until <= <DisbandInterval<T>>::get().FiveHundred {
							return Err(Error::<T>::IsNotAskForDisbandTime)?
						}
					},
					GroupMaxMembers::TenThousand => {
						if until <= <DisbandInterval<T>>::get().TenThousand {
							return Err(Error::<T>::IsNotAskForDisbandTime)?
						}
					},
					GroupMaxMembers::NoLimit =>
						if until <= <DisbandInterval<T>>::get().NoLimit {
							return Err(Error::<T>::IsNotAskForDisbandTime)?
						},
				}
			}

			ensure!(!(Self::is_voting(room.clone())), Error::<T>::IsVoting);
			room.disband_vote = DisbandVote::default();
			let disband_payment = Percent::from_percent(10) * room.create_payment.clone();
			let to = Self::treasury_id();
			T::NativeCurrency::transfer(&who, &to, disband_payment, KeepAlive)?;
			room.disband_vote_end_block = Self::now() + T::VoteExpire::get();
			room.disband_vote.approve_man.insert(who.clone());
			let user_consume_amount = Self::get_user_consume_amount(who.clone(), room.clone());
			room.disband_vote.approve_total_amount = room
				.disband_vote
				.approve_total_amount
				.checked_add(&user_consume_amount)
				.ok_or(Error::<T>::Overflow)?;
			<AllRoom<T>>::insert(group_id, room.clone());
			Self::judge(room.clone());

			Self::deposit_event(Event::AskForDisband(who.clone(), group_id));
			Ok(())
		}

		/// Set the price of the audio
		///
		/// The Origin must be Root.
		#[pallet::weight(10_000)]
		pub fn set_audio_price(
			origin: OriginFor<T>,
			cost: AudioPrice<BalanceOf<T>>,
		) -> DispatchResult {
			ensure_root(origin)?;

			<AudioPayment<T>>::put(cost);
			Self::deposit_event(Event::SetAudioPrice);
			Ok(())
		}

		/// Set the price of the props.
		///
		/// The Origin must be Root.
		#[pallet::weight(10_000)]
		pub fn set_props_price(
			origin: OriginFor<T>,
			cost: PropsPrice<BalanceOf<T>>,
		) -> DispatchResult {
			ensure_root(origin)?;

			<PropsPayment<T>>::put(cost);
			Self::deposit_event(Event::SetPropsPrice);
			Ok(())
		}

		/// Sets the interval between removes someone in the room.
		///
		/// The Origin must be Root.
		#[pallet::weight(10_000)]
		pub fn set_remove_interval(
			origin: OriginFor<T>,
			time: RemoveTime<T::BlockNumber>,
		) -> DispatchResult {
			ensure_root(origin)?;

			<RemoveInterval<T>>::put(time);
			Self::deposit_event(Event::SetKickInterval);
			Ok(())
		}

		/// Set the interval at which the group is dismissed
		#[pallet::weight(10_000)]
		pub fn set_disband_interval(
			origin: OriginFor<T>,
			time: DisbandTime<T::BlockNumber>,
		) -> DispatchResult {
			ensure_root(origin)?;

			<DisbandInterval<T>>::put(time);
			Self::deposit_event(Event::SetDisbandInterval);
			Ok(())
		}

		/// Users vote for disbanding the room.
		#[pallet::weight(10_000)]
		pub fn vote(origin: OriginFor<T>, group_id: u64, vote: ListenVote) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);
			let mut room = <AllRoom<T>>::get(group_id).unwrap();
			/// the consume amount should not be zero.
			let user_consume_amount = Self::get_user_consume_amount(who.clone(), room.clone());
			if user_consume_amount == <BalanceOf<T>>::from(0u32) {
				return Err(Error::<T>::ConsumeAmountIsZero)?
			}

			if !Self::is_voting(room.clone()) {
				Self::remove_vote_info(room);
				return Err(Error::<T>::NotVoting)?
			}
			let now = Self::now();

			match vote {
				ListenVote::Approve => {
					if room.disband_vote.approve_man.get(&who).is_some() {
						return Err(Error::<T>::DuplicateVote)?
					}
					room.disband_vote.approve_total_amount = room
						.disband_vote
						.approve_total_amount
						.checked_add(&user_consume_amount)
						.ok_or(Error::<T>::Overflow)?;
					room.disband_vote.approve_man.insert(who.clone());
					if room.disband_vote.reject_man.get(&who).is_some() {
						room.disband_vote.reject_man.remove(&who);
						room.disband_vote.reject_total_amount = room
							.disband_vote
							.reject_total_amount
							.saturating_sub(user_consume_amount);
					}
				},
				ListenVote::Reject => {
					if room.disband_vote.reject_man.get(&who).is_some() {
						return Err(Error::<T>::DuplicateVote)?
					}
					room.disband_vote.reject_man.insert(who.clone());
					room.disband_vote.reject_total_amount = room
						.disband_vote
						.reject_total_amount
						.checked_add(&user_consume_amount)
						.ok_or(Error::<T>::Overflow)?;
					if room.disband_vote.approve_man.get(&who).is_some() {
						room.disband_vote.approve_total_amount = room
							.disband_vote
							.approve_total_amount
							.saturating_sub(user_consume_amount);
						room.disband_vote.approve_man.remove(&who);
					}
				},
			}

			<AllRoom<T>>::insert(group_id, room.clone());
			Self::judge(room.clone());

			Self::deposit_event(Event::DisbandVote(who.clone(), group_id));
			Ok(())
		}

		/// Users get their reward in disbanded rooms.
		///
		/// the reward status should be NotGet in rooms.
		#[pallet::weight(10_000)]
		pub fn pay_out(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut amount = <BalanceOf<T>>::from(0u32);
			ensure!(
				<AllListeners<T>>::contains_key(who.clone()) &&
					!<AllListeners<T>>::get(who.clone()).rooms.is_empty(),
				Error::<T>::NotInAnyRoom
			);

			let rooms = <AllListeners<T>>::get(who.clone()).rooms;
			let mut new_rooms = rooms.clone();

			for room in rooms.iter() {
				if room.1 == RewardStatus::NotGet {
					let group_id = room.0.clone();
					if !<AllRoom<T>>::contains_key(group_id) {
						let session_index = Self::get_session_index();
						let mut is_get = false;

						for i in 0..Self::depth() as usize {
							let cur_session = session_index.saturating_sub(i as u32);
							if <InfoOfDisbandedRoom<T>>::contains_key(cur_session, group_id) {
								let mut info = <InfoOfDisbandedRoom<T>>::get(cur_session, group_id);
								info.already_get_count = info.already_get_count.saturating_add(1);
								let reward = info.per_man_reward;

								amount = amount.saturating_add(reward.clone());
								info.already_get_reward =
									info.already_get_reward.saturating_add(reward.clone());
								<InfoOfDisbandedRoom<T>>::insert(
									cur_session,
									group_id,
									info.clone(),
								);
								T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
									&who, reward,
								));

								<ListenersOfRoom<T>>::mutate(group_id, |h| h.remove(&who));
								if info.already_get_count.clone() == info.total_person.clone() {
									<ListenersOfRoom<T>>::remove(group_id);
								}

								if let Some(pos) = rooms.iter().position(|h| h.0 == group_id) {
									new_rooms.remove(pos);
									new_rooms.insert(pos, (group_id, RewardStatus::Get));
								}

								is_get = true;
								break
							}
							if cur_session == 0 {
								break
							}
						}
						if !is_get {
							if let Some(pos) = rooms.iter().position(|h| h.0 == group_id) {
								new_rooms.remove(pos);
								new_rooms.insert(pos, (group_id, RewardStatus::Expire));
							}
						}
					}
				}
			}

			<AllListeners<T>>::mutate(who.clone(), |h| h.rooms = new_rooms);

			if amount.is_zero() {
				return Err(Error::<T>::RewardAmountIsZero)?
			} else {
				Self::deposit_event(Event::Payout(who.clone(), amount));
			}

			Ok(())
		}

		/// Users send red envelopes in the room.
		#[pallet::weight(10_000)]
		pub fn send_redpacket_in_room(
			origin: OriginFor<T>,
			group_id: u64,
			currency_id: CurrencyIdOf<T>,
			lucky_man_number: u32,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(lucky_man_number != 0u32, Error::<T>::ParamIsZero);
			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);
			ensure!(
				amount >=
					<BalanceOf<T>>::from(lucky_man_number)
						.checked_mul(&T::RedPacketMinAmount::get())
						.ok_or(Error::<T>::Overflow)?,
				Error::<T>::AmountTooLow
			);
			let redpacket_id = <RedPacketId<T>>::get();

			let redpacket = RedPacket {
				id: redpacket_id,
				currency_id,
				boss: who.clone(),
				total: amount.clone(),
				lucky_man_number,
				already_get_man: BTreeSet::<T::AccountId>::default(),
				min_amount_of_per_man: T::RedPacketMinAmount::get(),
				already_get_amount: <BalanceOf<T>>::from(0u32),
				end_time: Self::now() + T::RedPackExpire::get(),
			};

			let amount_u128 = amount.saturated_into::<u128>();
			if currency_id == T::GetNativeCurrencyId::get() {
				T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(
					&who,
					amount.clone(),
					WithdrawReasons::TRANSFER.into(),
					KeepAlive,
				)?);
			} else {
				let amount = amount_u128.saturated_into::<MultiBalanceOf<T>>();
				T::MultiCurrency::withdraw(currency_id, &who, amount)?;
			}
			let now_id = redpacket_id.checked_add(1).ok_or(Error::<T>::Overflow)?;
			<RedPacketId<T>>::put(now_id);
			<RedPacketOfRoom<T>>::insert(group_id, redpacket_id, redpacket);

			Self::deposit_event(Event::SendRedPocket(group_id, redpacket_id, amount_u128));
			Ok(())
		}

		/// Expired red packets obtained by the owner.
		#[pallet::weight(10_000)]
		pub fn give_back_expired_redpacket(origin: OriginFor<T>, group_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let _ = Self::room_must_exists(group_id)?;
			Self::remove_redpacket_by_room_id(group_id, false);
			Self::deposit_event(Event::ReturnExpiredRedpacket(group_id));
			Ok(())
		}

		/// Users exit the room
		#[pallet::weight(10_000)]
		pub fn exit(origin: OriginFor<T>, group_id: u64) -> DispatchResult {
			let user = ensure_signed(origin)?;

			ensure!(!Self::is_in_disbanding(group_id)?, Error::<T>::Disbanding);
			ensure!(Self::is_in_room(group_id, user.clone())?, Error::<T>::NotInRoom);
			let mut room = <AllRoom<T>>::get(group_id).unwrap();
			let number = room.now_members_number;
			let user_amount = room.total_balances - room.group_manager_balances;

			if number > 1 {
				let amount = user_amount /
					room.now_members_number.saturated_into::<BalanceOf<T>>() /
					4u32.saturated_into::<BalanceOf<T>>();
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&user, amount));
				room.total_balances = room.total_balances.saturating_sub(amount);
				let room = Self::remove_someone_in_room(user.clone(), room.clone());
				<AllRoom<T>>::insert(group_id, room);
			} else {
				let amount = room.total_balances;
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&user, amount));
				let listeners = <ListenersOfRoom<T>>::get(group_id);
				<AllRoom<T>>::remove(group_id);
				<ListenersOfRoom<T>>::remove(group_id);
				Self::remove_redpacket_by_room_id(group_id, true);
			}

			Self::deposit_event(Event::Exit(user, group_id));
			Ok(())
		}

		/// Disbanding the room.
		///
		/// The vote has been passed.
		///
		/// The Origin can be everyone.
		#[pallet::weight(10_000)]
		pub fn disband_room(origin: OriginFor<T>, group_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_can_disband(group_id)?, Error::<T>::NotDisbanding);
			let room = Self::room_must_exists(group_id)?;
			Self::disband(room);
			Self::deposit_event(Event::DisbandRoom(group_id, who));
			Ok(())
		}

		/// Council Members reject disband the room.
		#[pallet::weight(10_000)]
		pub fn council_reject_disband(origin: OriginFor<T>, group_id: u64) -> DispatchResult {
			T::HalfRoomCouncilOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			let disband_rooms = <DisbandingRooms<T>>::get();
			let room = Self::room_must_exists(group_id)?;
			if let Some(disband_time) = disband_rooms.get(&group_id) {
				/// before the room is disband.
				if Self::now() <= *disband_time {
					<DisbandingRooms<T>>::mutate(|h| h.remove(&group_id));
					Self::remove_vote_info(room);
				} else {
					return Err(Error::<T>::Disbanding)?
				}
			} else {
				return Err(Error::<T>::NotDisbanding)?
			}

			Self::deposit_event(Event::CouncilRejectDisband(group_id));
			Ok(())
		}

		/// Users get redpacket in the room.
		#[pallet::weight(10_000)]
		pub fn get_redpacket_in_room(
			origin: OriginFor<T>,
			amount_vec: Vec<(<T::Lookup as StaticLookup>::Source, BalanceOf<T>)>,
			group_id: u64,
			redpacket_id: u128,
		) -> DispatchResult {
			let mul = ensure_signed(origin)?;

			let mut members: Vec<<T::Lookup as StaticLookup>::Source> = vec![];
			for i in amount_vec.clone() {
				members.push(i.0)
			}
			Self::check_accounts(members)?;

			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigIdIsNone)?;
			ensure!(mul.clone() == multisig_id.clone(), Error::<T>::NotMultisigId);
			ensure!(
				<RedPacketOfRoom<T>>::contains_key(group_id, redpacket_id),
				Error::<T>::RedPacketNotExists
			);
			let mut redpacket = <RedPacketOfRoom<T>>::get(group_id, redpacket_id).unwrap();
			if redpacket.end_time.clone() < Self::now() {
				Self::remove_redpacket(group_id, redpacket.clone());
				return Err(Error::<T>::Expire)?
			}
			let mut total_amount = <BalanceOf<T>>::from(0u32);

			for (who, amount) in amount_vec {
				let who = T::Lookup::lookup(who)?;
				if Self::is_in_room(group_id, who.clone())? == false {
					continue
				}

				//			ensure!(amount >= T::RedPacketMinAmount::get(), Error::<T>::AmountTooLow);
				if amount < redpacket.min_amount_of_per_man.clone() {
					continue
				}

				if redpacket.total.clone().saturating_sub(redpacket.already_get_amount.clone()) <
					amount
				{
					continue
				}

				if redpacket.lucky_man_number.clone() <=
					redpacket.already_get_man.clone().len() as u32
				{
					break
				}

				if redpacket.already_get_man.clone().contains(&who) == true {
					continue
				}

				if redpacket.currency_id == T::GetNativeCurrencyId::get() {
					T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
						&who,
						amount.clone(),
					));
				} else {
					let amount_u128 = amount.clone().saturated_into::<u128>();
					T::MultiCurrency::deposit(
						redpacket.currency_id.clone(),
						&who,
						amount_u128.saturated_into::<MultiBalanceOf<T>>(),
					);
				}

				redpacket.already_get_man.insert(who.clone());
				redpacket.already_get_amount += amount.clone();

				if redpacket.already_get_man.clone().len() ==
					(redpacket.lucky_man_number.clone() as usize)
				{
					Self::remove_redpacket(group_id, redpacket.clone());
				} else {
					<RedPacketOfRoom<T>>::insert(group_id, redpacket_id, redpacket.clone());
				}

				if redpacket.already_get_amount.clone() == redpacket.total {
					<RedPacketOfRoom<T>>::remove(group_id, redpacket_id);
				}
				total_amount += amount;
			}

			Self::deposit_event(Event::GetRedPocket(group_id, redpacket_id, total_amount));
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			Self::remove_expire_disband_info();
			<NowSessionIndex<T>>::put(Self::get_session_index());
			0
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The Vec is empty.
		VecEmpty,
		/// Data type conversion error
		NumberCanNotConvert,
		/// The room is not exists.
		RoomNotExists,
		/// There was no one in the room.
		RoomMembersIsZero,
		/// It's not supposed to be you
		ShouldNotYourself,
		Overflow,
		/// You are already in the room
		InRoom,
		/// you are not in the room
		NotInRoom,
		/// It's not time to remove others.
		IsNotRemoveTime,
		IsVoting,
		NotVoting,
		/// The vote is Duplicate.
		DuplicateVote,
		IsNotAskForDisbandTime,
		/// You are not in any room.
		NotInAnyRoom,
		AmountTooLow,
		RedPacketNotExists,
		Expire,
		NotMultisigId,
		MultisigIdIsNone,
		MustHavePaymentType,
		AmountShouldDifferent,
		MembersNumberToMax,
		UnknownRoomType,
		DuplicateMember,
		NotServerId,
		ServerIdNotExists,
		DivByZero,
		NotRewardTime,
		/// You didn't buy anything
		ConsumeAmountIsZero,
		MaxMembersUnknown,
		RoomMembersToMax,
		RoomMaxNotDiff,
		InBlackList,
		NotInBlackList,
		InProtectedDuration,
		ThreshouldLenErr,
		BadOrigin,
		PrivateRoom,
		PrivacyNotChange,
		RoomManager,
		Disbanding,
		NotDisbanding,
		RoomFreeAmountTooLow,
		ParamIsZero,
		RewardAmountIsZero,
		BuyNothing,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SetMultisig,
		AirDroped(T::AccountId),
		CreatedRoom(T::AccountId, u64),
		Invited(T::AccountId, T::AccountId),
		IntoRoom(Option<T::AccountId>, T::AccountId, u64),
		RejectedInvite(T::AccountId, u64),
		ChangedPermission(T::AccountId, u64),
		BuyProps(T::AccountId),
		BuyAudio(T::AccountId),
		Kicked(T::AccountId, u64),
		AskForDisband(T::AccountId, u64),
		DisbandVote(T::AccountId, u64),
		Payout(T::AccountId, BalanceOf<T>),
		SendRedPocket(u64, u128, u128),
		GetRedPocket(u64, u128, BalanceOf<T>),
		JoinCostChanged(u64, BalanceOf<T>),
		SetPropsPrice,
		SetAudioPrice,
		SetDisbandInterval,
		SetKickInterval,
		SetCreateCost,
		SetServerId(T::AccountId),
		Exit(T::AccountId, u64),
		ManagerGetReward(T::AccountId, BalanceOf<T>, BalanceOf<T>),
		SetMaxNumberOfRoomMembers(T::AccountId, u32),
		RemoveSomeoneFromBlackList(T::AccountId, u64),
		SetRoomPrivacy(u64, bool),
		DisbandRoom(u64, T::AccountId),
		CouncilRejectDisband(u64),
		ReturnExpiredRedpacket(u64),
		ListenTest,
	}

	impl<T: Config> Pallet<T> {
		fn sort_account_id(
			who: Vec<T::AccountId>,
		) -> result::Result<Vec<T::AccountId>, DispatchError> {
			ensure!(who.clone().len() > 0 as usize, Error::<T>::VecEmpty);
			let mut new_who = vec![];
			let who_cp = who.clone();
			for i in who_cp.iter() {
				if new_who.len() == 0 as usize {
					new_who.insert(0, i.clone());
				} else {
					let mut index = 0;
					for j in new_who.iter() {
						if i >= j {
							ensure!(i != j, Error::<T>::DuplicateMember);
							index += 1;
						}
					}
					new_who.insert(index, i.clone());
				}
			}

			Ok(new_who)
		}

		fn check_accounts(
			members: Vec<<T::Lookup as StaticLookup>::Source>,
		) -> result::Result<Vec<T::AccountId>, DispatchError> {
			let mut new_members: Vec<T::AccountId> = vec![];
			for i in members {
				let target = T::Lookup::lookup(i)?;
				new_members.push(target);
			}
			Ok(new_members)
		}

		pub fn treasury_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		pub fn now() -> T::BlockNumber {
			<system::Module<T>>::block_number()
		}

		fn add_listener_info(yourself: T::AccountId, group_id: u64) {
			<ListenersOfRoom<T>>::mutate(group_id, |h| h.insert(yourself.clone()));
			<AllListeners<T>>::mutate(yourself.clone(), |h| {
				h.rooms.push((group_id, RewardStatus::default()))
			});
		}

		fn get_room_consume_amount(
			room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) -> BalanceOf<T> {
			let mut consume_total_amount = <BalanceOf<T>>::from(0u32);
			for (account_id, amount) in room.consume.clone().iter() {
				consume_total_amount = consume_total_amount.saturating_add(*amount);
			}

			consume_total_amount
		}

		/// (vote is end)
		fn is_in_disbanding(group_id: u64) -> result::Result<bool, DispatchError> {
			Self::room_must_exists(group_id)?;
			let disband_rooms = <DisbandingRooms<T>>::get();
			if let Some(disband_time) = disband_rooms.get(&group_id) {
				return Ok(true)
			}
			Ok(false)
		}

		pub fn room_must_exists(
			group_id: u64,
		) -> result::Result<
			GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
			DispatchError,
		> {
			let room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			Ok(room)
		}

		fn is_in_room(group_id: u64, who: T::AccountId) -> result::Result<bool, DispatchError> {
			Self::room_must_exists(group_id)?;
			let listeners = <ListenersOfRoom<T>>::get(group_id);
			if listeners.clone().len() == 0 {
				return Err(Error::<T>::RoomMembersIsZero)?
			}
			if listeners.contains(&who) {
				Ok(true)
			} else {
				Ok(false)
			}
		}

		fn join_do(
			who: T::AccountId,
			invitee: Option<T::AccountId>,
			group_id: u64,
		) -> DispatchResult {
			let room_info = <AllRoom<T>>::get(group_id.clone()).unwrap();
			let join_cost = room_info.join_cost;

			if invitee.clone().is_some() && (invitee.clone().unwrap_or_default() != who.clone()) {
				if join_cost != <BalanceOf<T>>::from(0u32) {
					T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(
						&who,
						join_cost.clone(),
						WithdrawReasons::TRANSFER.into(),
						KeepAlive,
					)?);
					Self::pay_for(group_id.clone(), join_cost);
				}
				Self::add_listener_info(invitee.clone().unwrap(), group_id.clone());
			} else {
				if join_cost != <BalanceOf<T>>::from(0u32) {
					T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(
						&who,
						join_cost.clone(),
						WithdrawReasons::TRANSFER.into(),
						KeepAlive,
					)?);
					Self::pay_for(group_id, join_cost);
				}
				Self::add_listener_info(who.clone(), group_id.clone());
			}

			let mut room_info = <AllRoom<T>>::get(group_id.clone()).unwrap();
			room_info.now_members_number += 1;
			<AllRoom<T>>::insert(group_id.clone(), room_info);

			Ok(())
		}

		fn pay_for(group_id: u64, join_cost: BalanceOf<T>) {
			let payment_manager_now = Percent::from_percent(5) * join_cost;
			let payment_manager_later = Percent::from_percent(5) * join_cost;
			let payment_room_later = Percent::from_percent(50) * join_cost;
			let payment_treasury = Percent::from_percent(40) * join_cost;
			let mut room_info = <AllRoom<T>>::get(group_id.clone()).unwrap();

			room_info.total_balances += payment_room_later;
			room_info.total_balances += payment_manager_later;

			room_info.group_manager_balances += payment_manager_later;
			let group_manager = room_info.group_manager.clone();
			<AllRoom<T>>::insert(group_id, room_info);

			T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
				&group_manager,
				payment_manager_now,
			));

			let teasury_id = Self::treasury_id();
			T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
				&teasury_id,
				payment_treasury,
			));
		}

		fn update_user_consume(
			who: T::AccountId,
			room_info: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
			amount: BalanceOf<T>,
		) {
			let mut room = room_info;
			let group_id = room.group_id;
			let mut new_who_consume: (T::AccountId, BalanceOf<T>);
			if let Some(pos) = room.consume.clone().iter().position(|h| h.0 == who.clone()) {
				let old_who_consume = room.consume.remove(pos);
				new_who_consume = (old_who_consume.0, old_who_consume.1.saturating_add(amount));
			} else {
				new_who_consume = (who.clone(), amount);
			}
			let mut index = 0;
			for per_consume in room.consume.iter() {
				if per_consume.1 < new_who_consume.1 {
					break
				}
				index += 1;
			}
			room.consume.insert(index, new_who_consume.clone());
			let mut consume = room.consume.clone();
			room.council = vec![];
			if consume.len() > T::CouncilMaxNumber::get() as usize {
				consume.split_off(T::CouncilMaxNumber::get() as usize);
				room.council = consume;
			} else {
				room.council = room.consume.clone();
			}
			if Self::is_voting(room.clone()) {
				let mut is_in_vote = false;
				let disband_vote_info = room.disband_vote.clone();
				if disband_vote_info.approve_man.get(&who).is_some() {
					room.disband_vote.approve_total_amount += amount;
					is_in_vote = true;
				}
				if disband_vote_info.reject_man.get(&who).is_some() {
					room.disband_vote.reject_total_amount += amount;
					is_in_vote = true;
				}
				if is_in_vote {
					<AllRoom<T>>::insert(group_id, room.clone());
					Self::judge(room.clone());
					return
				}
			}
			<AllRoom<T>>::insert(group_id, room);
		}

		fn is_voting(
			room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) -> bool {
			if !room.disband_vote_end_block.clone().is_zero() &&
				(Self::now().saturating_sub(room.disband_vote_end_block.clone()).is_zero())
			{
				return true
			}
			false
		}

		fn judge(
			room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) {
			let mut room = room.clone();
			let group_id = room.group_id;
			let now = Self::now();
			let vote_result = Self::is_vote_end(room.clone());

			if vote_result.0 == End {
				if vote_result.1 == Pass {
					if vote_result.2 == <BalanceOf<T>>::from(0u32) {
						Self::disband(room);
					} else {
						room.disband_vote_end_block = now;
						<AllRoom<T>>::insert(group_id, room);
						<DisbandingRooms<T>>::mutate(|h| {
							h.insert(group_id, now + T::DelayDisbandDuration::get())
						});
					}
				} else {
					Self::remove_vote_info(room);
				}
			}
		}

		fn is_vote_end(
			room_info: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) -> (bool, bool, BalanceOf<T>) {
			let end_time = room_info.disband_vote_end_block.clone();
			let approve_total_amount = room_info.disband_vote.approve_total_amount.clone();
			let reject_total_amount = room_info.disband_vote.reject_total_amount.clone();
			let consume_total_amount = Self::get_room_consume_amount(room_info.clone());
			if approve_total_amount * <BalanceOf<T>>::from(2u32) >= consume_total_amount ||
				reject_total_amount * <BalanceOf<T>>::from(2u32) >= consume_total_amount
			{
				if approve_total_amount * <BalanceOf<T>>::from(2u32) >= consume_total_amount {
					(End, Pass, consume_total_amount)
				} else {
					(End, NotPass, consume_total_amount)
				}
			} else {
				if end_time >= Self::now() {
					(NotEnd, NotPass, consume_total_amount)
				} else {
					(End, NotPass, consume_total_amount)
				}
			}
		}

		fn disband(
			room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) {
			let group_id = room.group_id;
			Self::remove_redpacket_by_room_id(group_id, true);
			let total_reward = room.total_balances.clone();
			let manager_reward = room.group_manager_balances.clone();
			T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
				&room.group_manager,
				manager_reward,
			));
			let listener_reward = total_reward.clone() - manager_reward.clone();
			let session_index = Self::get_session_index();
			let per_man_reward =
				listener_reward.clone() / <BalanceOf<T>>::from(room.now_members_number);
			let room_rewad_info = RoomRewardInfo {
				total_person: room.now_members_number.clone(),
				already_get_count: 0u32,
				total_reward: listener_reward.clone(),
				already_get_reward: <BalanceOf<T>>::from(0u32),
				per_man_reward: per_man_reward.clone(),
			};

			<InfoOfDisbandedRoom<T>>::insert(session_index, group_id, room_rewad_info);
			<ListenersOfRoom<T>>::remove(group_id);
			<AllRoom<T>>::remove(group_id);
			<DisbandingRooms<T>>::mutate(|h| h.remove(&group_id));
			T::CollectiveHandler::remove_room_collective_info(group_id);
			T::RoomTreasuryHandler::remove_room_treasury_info(group_id);
		}

		fn remove_vote_info(
			mut room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) {
			room.disband_vote = <DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>>::default();
			let now = Self::now();
			if room.disband_vote_end_block > now {
				room.disband_vote_end_block = now;
			}

			<AllRoom<T>>::insert(room.group_id.clone(), room);
		}

		fn get_session_index() -> SessionIndex {
			let now = Self::now().saturated_into::<SessionIndex>();
			let session_index = now / EPOCH_DURATION_IN_BLOCKS;
			session_index
		}

		fn remove_redpacket_by_room_id(room_id: u64, all: bool) {
			let redpackets = <RedPacketOfRoom<T>>::iter_prefix(room_id).collect::<Vec<_>>();
			let now = Self::now();

			if all {
				for redpacket in redpackets.iter() {
					Self::remove_redpacket(room_id, redpacket.1.clone());
				}
			} else {
				for redpacket in redpackets.iter() {
					if redpacket.1.end_time < now {
						Self::remove_redpacket(room_id, redpacket.1.clone());
					}
				}
			}
		}

		fn remove_redpacket(
			room_id: u64,
			redpacket: RedPacket<
				T::AccountId,
				BTreeSet<T::AccountId>,
				BalanceOf<T>,
				T::BlockNumber,
				CurrencyIdOf<T>,
			>,
		) {
			let who = redpacket.boss.clone();
			let currency_id = redpacket.currency_id.clone();
			let remain = redpacket.total.clone() - redpacket.already_get_amount.clone();
			let redpacket_id = redpacket.id.clone();
			let remain_u128 = remain.saturated_into::<u128>();
			if currency_id == T::GetNativeCurrencyId::get() {
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&who, remain));
			} else {
				let amount = remain_u128.saturated_into::<MultiBalanceOf<T>>();
				T::MultiCurrency::deposit(currency_id, &who, amount);
			}
			<RedPacketOfRoom<T>>::remove(room_id, redpacket_id);
		}

		fn remove_someone_in_room(
			who: T::AccountId,
			room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) -> GroupInfo<
			T::AccountId,
			BalanceOf<T>,
			AllProps,
			Audio,
			T::BlockNumber,
			GroupMaxMembers,
			DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
			T::Moment,
		> {
			let mut room = room;
			room.now_members_number = room.now_members_number.saturating_sub(1);
			let mut room = Self::remove_consumer_info(room, who.clone());
			let mut listeners = <ListenersOfRoom<T>>::get(room.group_id.clone());
			let _ = listeners.take(&who);
			if room.clone().group_manager == who.clone() {
				if room.consume.len() > 0 {
					let mut consume = room.consume.clone();
					room.group_manager = consume.remove(0).0;
				} else {
					room.group_manager = listeners.clone().iter().next().unwrap().clone();
				}
			}
			<ListenersOfRoom<T>>::insert(room.group_id.clone(), listeners);

			room
		}

		fn remove_consumer_info(
			mut room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
			who: T::AccountId,
		) -> GroupInfo<
			T::AccountId,
			BalanceOf<T>,
			AllProps,
			Audio,
			T::BlockNumber,
			GroupMaxMembers,
			DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
			T::Moment,
		> {
			if let Some(pos) = room.consume.iter().position(|h| h.0 == who.clone()) {
				room.consume.remove(pos);
			}
			room.council = vec![];
			let mut consume = room.consume.clone();
			if consume.len() > T::CouncilMaxNumber::get() as usize {
				consume.split_off(T::CouncilMaxNumber::get() as usize);
				room.council = consume;
			} else {
				room.council = room.consume.clone();
			}
			room
		}

		fn get_user_consume_amount(
			who: T::AccountId,
			room: GroupInfo<
				T::AccountId,
				BalanceOf<T>,
				AllProps,
				Audio,
				T::BlockNumber,
				GroupMaxMembers,
				DisbandVote<BTreeSet<T::AccountId>, BalanceOf<T>>,
				T::Moment,
			>,
		) -> BalanceOf<T> {
			let mut room = room.clone();
			if let Some(pos) = room.consume.clone().iter().position(|h| h.0 == who) {
				let consume = room.consume.remove(pos);
				return consume.1
			} else {
				return <BalanceOf<T>>::from(0u32)
			}
		}

		pub fn is_can_disband(group_id: u64) -> result::Result<bool, DispatchError> {
			Self::room_must_exists(group_id)?;
			let disband_rooms = <DisbandingRooms<T>>::get();
			if let Some(disband_time) = disband_rooms.get(&group_id) {
				if Self::now() >= *disband_time {
					return Ok(true)
				}
			}
			Ok(false)
		}

		/// Delete information about rooms that have been dismissed
		fn remove_expire_disband_info() {
			let mut index: u32;
			let mut info: Vec<(u64, RoomRewardInfo<BalanceOf<T>>)>;

			let last_session_index = Self::last_session_index();
			let now_session = Self::get_session_index();

			if now_session - last_session_index <= Self::depth() {
				return
			}

			index = last_session_index;
			info = <InfoOfDisbandedRoom<T>>::iter_prefix(last_session_index).collect::<Vec<_>>();

			if info.len() == 0 {
				let next = last_session_index.saturating_add(1);
				let now_session = Self::get_session_index();
				if now_session - next <= Self::depth() {
					return
				}
				info = <InfoOfDisbandedRoom<T>>::iter_prefix(next).collect::<Vec<_>>();
				index = next;
			}

			info.truncate(200);
			for i in info.iter() {
				let group_id = i.0;
				<ListenersOfRoom<T>>::remove(group_id);
				let disband_room_info = <InfoOfDisbandedRoom<T>>::get(index, group_id);
				let remain_reward =
					disband_room_info.total_reward - disband_room_info.already_get_reward;
				let teasury_id = Self::treasury_id();
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(
					&teasury_id,
					remain_reward,
				));
				<InfoOfDisbandedRoom<T>>::remove(index, group_id);
			}
			<LastSessionIndex<T>>::put(index);
		}
	}

	impl<T: Config> ListenHandler<u64, T::AccountId, DispatchError, u128> for Pallet<T> {
		fn get_room_council(
			room_id: u64,
		) -> Result<Vec<<T as frame_system::Config>::AccountId>, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = Self::room_must_exists(room_id)?;
			let council_and_amount = room_info.council;
			let mut council = vec![];
			for i in council_and_amount.iter() {
				council.push(i.0.clone());
			}
			council.sort();
			Ok(council)
		}

		fn get_prime(
			room_id: u64,
		) -> Result<Option<<T as frame_system::Config>::AccountId>, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = Self::room_must_exists(room_id)?;
			let prime = room_info.prime;
			Ok(prime)
		}

		fn get_root(room_id: u64) -> Result<<T as frame_system::Config>::AccountId, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = Self::room_must_exists(room_id)?;
			let root = room_info.group_manager;
			Ok(root)
		}

		fn get_room_free_amount(room_id: u64) -> u128 {
			if let Some(room) = <AllRoom<T>>::get(room_id) {
				let disband_rooms = <DisbandingRooms<T>>::get();
				if let Some(disband_time) = disband_rooms.get(&room_id) {
					if Self::now() >= *disband_time {
						return 0u128
					}
				}

				let free_amount = room.total_balances.saturating_sub(room.group_manager_balances);
				free_amount.saturated_into::<u128>()
			} else {
				return 0u128
			}
		}

		fn sub_room_free_amount(room_id: u64, amount: u128) -> Result<(), DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let mut room = Self::room_must_exists(room_id)?;
			if room.total_balances.saturating_sub(room.group_manager_balances) >=
				amount.saturated_into::<BalanceOf<T>>()
			{
				room.total_balances = room.total_balances - amount.saturated_into::<BalanceOf<T>>();
				<AllRoom<T>>::insert(room_id, room);
			} else {
				return Err(Error::<T>::RoomFreeAmountTooLow)?
			}
			Ok(())
		}
	}
}
