// Copyright 2021-2022 LISTEN TEAM..
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
pub mod primitives;
pub mod room_id;

pub use crate::pallet::*;
use crate::primitives::{
	vote, AllProps, Audio, AudioPrice, DisbandVote, GroupInfo, GroupMaxMembers, ListenVote,
	PersonInfo, PropsPrice, RedPacket, RewardStatus, RoomRewardInfo, SessionIndex,
};

#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;
pub use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{
		BalanceStatus as Status, Currency, EnsureOrigin,
		ExistenceRequirement::{AllowDeath, KeepAlive},
		Get, OnUnbalanced, ReservableCurrency, WithdrawReasons,
	},
	transactional,
	weights::Weight,
	Blake2_256, IterableStorageDoubleMap, IterableStorageMap, PalletId,
};
pub use frame_system::{self as system, ensure_root, ensure_signed};
use listen_primitives::{
	constants::{currency::*, time::*},
	traits::{CollectiveHandler, ListenHandler, RoomTreasuryHandler},
	*,
};
use orml_traits::MultiCurrency;
use pallet_multisig;
use pallet_timestamp as timestamp;
pub use room_id::{AccountIdConversion as RoomTreasuryIdConvertor, RoomTreasuryId};
use sp_runtime::{
	traits::{
		AccountIdConversion, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Saturating,
		StaticLookup, Zero,
	},
	DispatchError, DispatchResult, Percent, SaturatedConversion,
};
use sp_std::{
	collections::{btree_map::BTreeMap as BTMap, btree_set::BTreeSet},
	convert::{TryFrom, TryInto},
	prelude::*,
	result,
};
use vote::*;

pub type RoomId = u64;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::{
		Blake2_128Concat, IsType, StorageDoubleMap, StorageMap, StorageValue, ValueQuery,
	};
	use frame_system::pallet_prelude::*;

	pub(crate) type MultiBalanceOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	pub(crate) type CurrencyIdOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
		<T as frame_system::Config>::AccountId,
	>>::CurrencyId;
	pub(crate) type RoomInfoOf<T> = GroupInfo<
		<T as system::Config>::AccountId,
		MultiBalanceOf<T>,
		AllProps,
		Audio,
		<T as system::Config>::BlockNumber,
		DisbandVote<BTreeSet<<T as system::Config>::AccountId>, MultiBalanceOf<T>>,
		<T as timestamp::Config>::Moment,
	>;

	#[pallet::config]
	#[pallet::disable_frame_system_supertrait_check]
	pub trait Config: system::Config + timestamp::Config + pallet_multisig::Config {
		type Event: From<Event<Self>>
			+ Into<<Self as system::Config>::Event>
			+ IsType<<Self as frame_system::Config>::Event>;
		type MultiCurrency: MultiCurrency<Self::AccountId>;
		type CollectiveHandler: CollectiveHandler<u64, Self::BlockNumber, DispatchError>;
		type RoomRootOrigin: EnsureOrigin<Self::Origin>;
		type RoomRootOrHalfCouncilOrigin: EnsureOrigin<Self::Origin>;
		type RoomRootOrHalfRoomCouncilOrSomeRoomCouncilOrigin: EnsureOrigin<Self::Origin>;
		type HalfRoomCouncilOrigin: EnsureOrigin<Self::Origin>;
		type RoomTreasuryHandler: RoomTreasuryHandler<u64>;
		type SetMultisigOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;
		// type RoomIdConvert: RoomIdConvertor<Self::AccountId>;
		#[pallet::constant]
		type VoteExpire: Get<Self::BlockNumber>;
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
		type AirDropAmount: Get<MultiBalanceOf<Self>>;
		/// LT currency id in the tokens module
		#[pallet::constant]
		type GetNativeCurrencyId: Get<CurrencyIdOf<Self>>;
		#[pallet::constant]
		type GetLikeCurrencyId: Get<CurrencyIdOf<Self>>;
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
	pub type NextGroupId<T: Config> = StorageValue<_, RoomId, ValueQuery>;

	/// The cost of creating a group
	#[pallet::storage]
	#[pallet::getter(fn create_cost)]
	pub type CreatePayment<T: Config> =
		StorageMap<_, Blake2_128Concat, GroupMaxMembers, MultiBalanceOf<T>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn red_packet_id)]
	pub type NextRedPacketId<T: Config> = StorageValue<_, u128, ValueQuery>;

	/// All groups that have been created
	#[pallet::storage]
	#[pallet::getter(fn all_room)]
	pub type AllRoom<T: Config> = StorageMap<_, Blake2_128Concat, RoomId, RoomInfoOf<T>>;

	/// Everyone in the room (People who have not yet claimed their reward).
	#[pallet::storage]
	#[pallet::getter(fn listeners_of_room)]
	pub type ListenersOfRoom<T: Config> =
		StorageMap<_, Blake2_128Concat, RoomId, BTreeSet<T::AccountId>, ValueQuery>;

	/// Minimum amount of red packets per person in each currency.
	#[pallet::storage]
	#[pallet::getter(fn min_redpack_amount)]
	pub type MinRedPackAmount<T: Config> =
		StorageMap<_, Blake2_128Concat, CurrencyIdOf<T>, MultiBalanceOf<T>, ValueQuery>;

	/// Specific information about each person's purchase
	#[pallet::storage]
	#[pallet::getter(fn purchase_summary)]
	pub type PurchaseSummary<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		PersonInfo<AllProps, Audio, MultiBalanceOf<T>, RewardStatus>,
		ValueQuery,
	>;

	/// Disbanded rooms
	#[pallet::storage]
	#[pallet::getter(fn info_of_disband_room)]
	pub type InfoOfDisbandedRoom<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		SessionIndex,
		Blake2_128Concat,
		RoomId,
		RoomRewardInfo<MultiBalanceOf<T>>,
		ValueQuery,
	>;

	/// All the red packets in the group
	#[pallet::storage]
	#[pallet::getter(fn red_packets_of_room)]
	pub type RedPacketOfRoom<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		RoomId,
		Blake2_128Concat,
		u128,
		RedPacket<
			T::AccountId,
			BTreeSet<T::AccountId>,
			MultiBalanceOf<T>,
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
	#[pallet::storage]
	#[pallet::getter(fn pending_disband_rooms)]
	pub type PendingDisbandRooms<T: Config> =
		StorageValue<_, BTMap<u64, T::BlockNumber>, ValueQuery>;

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

	#[pallet::storage]
	#[pallet::getter(fn remove_interval)]
	pub type RemoveInterval<T: Config> =
		StorageMap<_, Blake2_128Concat, GroupMaxMembers, T::BlockNumber, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn disband_interval)]
	pub type DisbandInterval<T: Config> =
		StorageMap<_, Blake2_128Concat, GroupMaxMembers, T::BlockNumber, ValueQuery>;

	#[pallet::type_value]
	pub fn PropsPaymentOnEmpty<T: Config>() -> PropsPrice<MultiBalanceOf<T>> {
		PropsPrice {
			picture: <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(3) * UNIT,
			)
			.ok()
			.unwrap(),
			text: <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(1) * UNIT,
			)
			.ok()
			.unwrap(),
			video: <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(3) * UNIT,
			)
			.ok()
			.unwrap(),
		}
	}

	#[pallet::storage]
	#[pallet::getter(fn props_payment)]
	pub type PropsPayment<T: Config> =
		StorageValue<_, PropsPrice<MultiBalanceOf<T>>, ValueQuery, PropsPaymentOnEmpty<T>>;

	#[pallet::type_value]
	pub fn AudioPaymentOnEmpty<T: Config>() -> AudioPrice<MultiBalanceOf<T>> {
		AudioPrice {
			ten_seconds: <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(1) * UNIT,
			)
			.ok()
			.unwrap(),
			thirty_seconds: <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(2) * UNIT,
			)
			.ok()
			.unwrap(),
			minutes: <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(
				Percent::from_percent(2) * UNIT,
			)
			.ok()
			.unwrap(),
		}
	}
	#[pallet::storage]
	#[pallet::getter(fn audio_payment)]
	pub type AudioPayment<T: Config> =
		StorageValue<_, AudioPrice<MultiBalanceOf<T>>, ValueQuery, AudioPaymentOnEmpty<T>>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub server_id: Option<T::AccountId>,
		pub multisig_members: Vec<T::AccountId>,
	}
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { server_id: None, multisig_members: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			if let Some(server_id) = &self.server_id {
				ServerId::<T>::put(server_id);
			}

			if self.multisig_members.len() >= 2 {
				let mut members = self.multisig_members.clone();
				members.sort();
				let threshould = 1u16;
				let multisig_id =
					<pallet_multisig::Pallet<T>>::multi_account_id(&members, threshould);
				<Multisig<T>>::put((members, threshould, multisig_id));
			}

			{
				CreatePayment::<T>::insert(
					GroupMaxMembers::Ten,
					UNIT.saturated_into::<MultiBalanceOf<T>>(),
				);
				CreatePayment::<T>::insert(
					GroupMaxMembers::Hundred,
					(10 * UNIT).saturated_into::<MultiBalanceOf<T>>(),
				);
				CreatePayment::<T>::insert(
					GroupMaxMembers::FiveHundred,
					(30 * UNIT).saturated_into::<MultiBalanceOf<T>>(),
				);
				CreatePayment::<T>::insert(
					GroupMaxMembers::TenThousand,
					(200 * UNIT).saturated_into::<MultiBalanceOf<T>>(),
				);
				CreatePayment::<T>::insert(
					GroupMaxMembers::NoLimit,
					(1000 * UNIT).saturated_into::<MultiBalanceOf<T>>(),
				);
			}

			{
				RemoveInterval::<T>::insert(GroupMaxMembers::Ten, T::BlockNumber::from(7 * DAYS));
				RemoveInterval::<T>::insert(
					GroupMaxMembers::Hundred,
					T::BlockNumber::from(1 * DAYS),
				);
				RemoveInterval::<T>::insert(
					GroupMaxMembers::FiveHundred,
					T::BlockNumber::from(12 * HOURS),
				);
				RemoveInterval::<T>::insert(
					GroupMaxMembers::TenThousand,
					T::BlockNumber::from(8 * HOURS),
				);
				RemoveInterval::<T>::insert(
					GroupMaxMembers::NoLimit,
					T::BlockNumber::from(4 * HOURS),
				);
			}

			{
				DisbandInterval::<T>::insert(GroupMaxMembers::Ten, T::BlockNumber::from(1 * DAYS));
				DisbandInterval::<T>::insert(
					GroupMaxMembers::Hundred,
					T::BlockNumber::from(7 * DAYS),
				);
				DisbandInterval::<T>::insert(
					GroupMaxMembers::FiveHundred,
					T::BlockNumber::from(15 * DAYS),
				);
				DisbandInterval::<T>::insert(
					GroupMaxMembers::TenThousand,
					T::BlockNumber::from(30 * DAYS),
				);
				DisbandInterval::<T>::insert(
					GroupMaxMembers::NoLimit,
					T::BlockNumber::from(60 * DAYS),
				);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set a multi-sign account
		///
		/// The Origin must be one of the LISTEN Foundation accounts.
		#[pallet::weight(1500_000_000)]
		pub fn set_multisig(
			origin: OriginFor<T>,
			members: Vec<<T::Lookup as StaticLookup>::Source>,
			threshould: u16,
		) -> DispatchResult {
			T::SetMultisigOrigin::ensure_origin(origin)?;

			let len = members.len();
			ensure!(
				len > 1 && len <= 20 && threshould > 0u16 && threshould <= len as u16,
				Error::<T>::ThreshouldOrLenErr
			);

			let mut members = Self::check_accounts(members)?;
			members.sort();
			ensure!(Self::is_unique(members.clone()), Error::<T>::NotUnique);

			let multisig_id = pallet_multisig::Pallet::<T>::multi_account_id(&members, threshould);
			<Multisig<T>>::put((members, threshould, multisig_id));

			Self::deposit_event(Event::SetMultisig);
			Ok(())
		}

		/// User gets airdrop.
		///
		/// The Origin can be everyone, but your account balances should be zero.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn air_drop(
			origin: OriginFor<T>,
			members: Vec<<T::Lookup as StaticLookup>::Source>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let members = Self::check_accounts(members)?;
			ensure!(members.len() as u32 <= 100u32, Error::<T>::VecTooLarge);
			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigNotExists)?;
			ensure!(who == multisig_id, Error::<T>::NotMultisigId);

			for user in members.iter() {
				ensure!(
					T::MultiCurrency::total_balance(T::GetNativeCurrencyId::get(), &user).is_zero(),
					Error::<T>::BalanceIsNotZero
				);
				T::MultiCurrency::deposit(
					T::GetNativeCurrencyId::get(),
					&user,
					T::AirDropAmount::get(),
				)?;
				<system::Pallet<T>>::inc_consumers(&user)?;
			}

			Self::deposit_event(Event::AirDroped(members.len() as u8, members));
			Ok(())
		}

		/// The user creates a room.
		///
		/// Everyone can do it, and he(she) will be room manager.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn create_room(
			origin: OriginFor<T>,
			max_members: u32,
			group_type: Vec<u8>,
			join_cost: MultiBalanceOf<T>,
			is_private: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let create_payment = Self::create_cost(&Self::members_convert_type(max_members)?);

			// the create cost transfers to the treasury
			let to = Self::treasury_id();
			T::MultiCurrency::transfer(T::GetNativeCurrencyId::get(), &who, &to, create_payment)?;
			let group_id = <NextGroupId<T>>::get();

			let now = Self::now();

			let group_info = GroupInfo {
				group_id,
				room_treasury_id: RoomTreasuryId(group_id).into_account(),
				create_payment,
				last_block_of_get_the_reward: now,
				group_manager: Some(who.clone()),
				prime: None,
				max_members,
				group_type,
				join_cost,
				props: AllProps::default(),
				audio: Audio::default(),
				total_balances: <MultiBalanceOf<T>>::from(0u32),
				group_manager_balances: <MultiBalanceOf<T>>::from(0u32),
				now_members_number: 1u32,
				last_remove_someone_block: now,
				disband_vote_end_block: now,
				disband_vote: DisbandVote::default(),
				create_time: <timestamp::Pallet<T>>::get(),
				create_block: Self::now(),
				consume: vec![],
				council: vec![],
				black_list: vec![],
				is_private,
			};

			<AllRoom<T>>::insert(group_id, group_info);
			Self::add_new_user_info(&who, group_id);
			<NextGroupId<T>>::try_mutate(|h| -> DispatchResult {
				*h = h.checked_add(1u64).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			Self::deposit_event(Event::CreatedRoom(who, group_id));
			Ok(())
		}


		#[pallet::weight(1500_000_000)]
		pub fn set_council_members(origin: OriginFor<T>, group_id: RoomId, members: Vec<T::AccountId>) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;
			ensure!(members.len() as u32 > 0u32, Error::<T>::VecEmpty);
			AllRoom::<T>::try_mutate_exists(group_id, |room| -> DispatchResult {
				let mut room_info = room.take().ok_or(Error::<T>::RoomNotExists)?;
				room_info.council = members;
				*room = Some(room_info);
				Ok(())
			})?;
			Self::deposit_event(Event::SetCouncilMembers(group_id));
			Ok(())
		}


		#[pallet::weight(1500_000_000)]
		pub fn add_council_member(origin: OriginFor<T>, group_id: RoomId, new_member: T::AccountId) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;
			AllRoom::<T>::try_mutate_exists(group_id, |room| -> DispatchResult {
				let mut room_info = room.take().ok_or(Error::<T>::RoomNotExists)?;
				if let None = room_info.council.iter().position(|h| h == &new_member) {
					room_info.council.push(new_member.clone());
				} else {
					return Err(Error::<T>::MemberAlreadyInCouncil)?;
				}
				*room = Some(room_info);
				Ok(())
			})?;
			Self::deposit_event(Event::AddCouncilMember(group_id, new_member));
			Ok(())
		}


		#[pallet::weight(1500_000_000)]
		pub fn remove_council_member(origin: OriginFor<T>, group_id: RoomId, who: T::AccountId) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;
			AllRoom::<T>::try_mutate_exists(group_id, |room| -> DispatchResult {
				let mut room_info = room.take().ok_or(Error::<T>::RoomNotExists)?;
				room_info.council.retain( |h| h != &who);
				*room = Some(room_info);
				Ok(())
			})?;
			Self::deposit_event(Event::RemoveCouncilMember(group_id, who));
			Ok(())
		}


		/// The room manager get his reward.
		/// You should have already created the room.
		/// Receive rewards for a fixed period of time
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn manager_get_reward(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			<AllRoom<T>>::try_mutate_exists(group_id, |room| -> DispatchResult {
				let mut room_info = room.take().ok_or(Error::<T>::RoomNotExists)?;
				let (total_reward, manager_proportion_amount, room_proportion_amount, last_block) =
					Self::get_reward_info(&room_info)?;

				let group_manager = match room_info.group_manager.clone() {
					None => return Err(Error::<T>::RoomManagerNotExists)?,
					Some(x) => x,
				};
				T::MultiCurrency::deposit(
					T::GetNativeCurrencyId::get(),
					&group_manager,
					manager_proportion_amount,
				)?;

				room_info.total_balances = room_info
					.total_balances
					.clone()
					.checked_add(&room_proportion_amount)
					.ok_or(Error::<T>::Overflow)?;
				room_info.total_balances =
					room_info.total_balances.saturating_sub(manager_proportion_amount);
				room_info.last_block_of_get_the_reward = last_block;
				*room = Some(room_info);
				Self::deposit_event(Event::ManagerGetReward(group_manager, total_reward));
				Ok(())
			})
		}

		/// The room manager modify the cost of group entry.
		///
		/// The Origin must be RoomManager.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn update_join_cost(
			origin: OriginFor<T>,
			group_id: RoomId,
			join_cost: MultiBalanceOf<T>,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			AllRoom::<T>::try_mutate_exists(group_id, |room| -> DispatchResult {
				let mut room_info = room.take().ok_or(Error::<T>::RoomNotExists)?;
				ensure!(room_info.join_cost != join_cost, Error::<T>::AmountShouldDifferent);
				room_info.join_cost = join_cost;
				*room = Some(room_info);
				Self::deposit_event(Event::JoinCostChanged(group_id, join_cost));
				Ok(())
			})
		}

		/// The user joins the room.
		///
		/// If invitee is None, you go into the room by yourself. Otherwise, you invite people in.
		/// You can not invite yourself.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn into_room(
			origin: OriginFor<T>,
			group_id: RoomId,
			invitee: Option<<T::Lookup as StaticLookup>::Source>,
		) -> DispatchResult {
			let inviter = ensure_signed(origin)?;

			let invitee = match invitee {
				None => None,
				Some(x) => Some(T::Lookup::lookup(x)?),
			};

			let room_info = AllRoom::<T>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;

			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);

			// Private rooms must be invited by the manager.
			if room_info.is_private {
				ensure!(
					invitee.is_some() && Some(inviter.clone()) == room_info.group_manager,
					Error::<T>::PrivateRoom
				);
			}

			let invitee = invitee.unwrap_or_else(|| inviter.clone());

			// people who into the room must be not in blacklist.
			let black_list = room_info.black_list;
			if black_list.iter().position(|h| h == &invitee).is_some() {
				return Err(Error::<T>::InBlackList)?
			};

			ensure!(
				room_info.max_members >= room_info.now_members_number.saturating_add(1u32),
				Error::<T>::MembersNumberToMax
			);
			ensure!(
				Self::room_exists_and_user_in_room(group_id, &invitee).is_err(),
				Error::<T>::InRoom
			);

			Self::join_do(&inviter, &invitee, group_id)?;

			Self::deposit_event(Event::IntoRoom(invitee, inviter, group_id));
			Ok(())
		}

		/// Set the privacy properties of the room
		///
		/// The Origin must be room manager.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn set_room_privacy(
			origin: OriginFor<T>,
			room_id: RoomId,
			is_private: bool,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			AllRoom::<T>::try_mutate_exists(room_id, |r| -> DispatchResult {
				let mut room = r.as_mut().take().ok_or(Error::<T>::RoomNotExists)?;
				ensure!(room.is_private != is_private, Error::<T>::PrivacyNotChange);
				room.is_private = is_private;
				*r = Some(room.clone());

				Self::deposit_event(Event::SetRoomPrivacy(room_id, is_private));
				Ok(())
			})
		}

		/// Set the maximum number of people in the room
		///
		/// The Origin must be room manager.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn set_max_number_for_room_members(
			origin: OriginFor<T>,
			group_id: RoomId,
			new_max: u32,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			let mut room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let manager = match room.group_manager.clone() {
				None => return Err(Error::<T>::RoomManagerNotExists)?,
				Some(x) => x,
			};
			let old_number = room.now_members_number;
			let old_max = room.max_members;

			ensure!(old_max != new_max, Error::<T>::RoomMaxNotDiff);
			// The value cannot be smaller than the current group size
			ensure!(old_number <= new_max, Error::<T>::RoomMembersToMax);

			let new_amount = Self::create_cost(&Self::members_convert_type(new_max)?);
			let add_amount = new_amount
				.saturating_sub(Self::create_cost(&Self::members_convert_type(old_max)?))
				.saturated_into::<MultiBalanceOf<T>>();

			if add_amount != Zero::zero() {
				// transfers amount to the treasury.
				let to = Self::treasury_id();
				T::MultiCurrency::transfer(
					T::GetNativeCurrencyId::get(),
					&manager,
					&to,
					add_amount,
				)?;
			}

			room.max_members = new_max;
			room.create_payment = new_amount;
			<AllRoom<T>>::insert(group_id, room);

			Self::deposit_event(Event::SetMaxNumberOfRoomMembers(manager, new_max));
			Ok(())
		}

		/// Users buy props in the room.
		///
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn buy_props_in_room(
			origin: OriginFor<T>,
			group_id: RoomId,
			props: AllProps,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			let mut dollars = <MultiBalanceOf<T>>::from(0u32);
			ensure!(
				props.picture != 0u32 || props.text != 0u32 || props.video != 0u32,
				Error::<T>::BuyNothing
			);
			let props_cost = <PropsPayment<T>>::get();

			if props.picture > 0u32 {
				dollars = props_cost
					.picture
					.checked_mul(&<MultiBalanceOf<T>>::from(props.picture))
					.ok_or(Error::<T>::Overflow)?;
			}

			if props.text > 0u32 {
				dollars = dollars
					.checked_add(
						&props_cost
							.text
							.checked_mul(&<MultiBalanceOf<T>>::from(props.text))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

			if props.video > 0u32 {
				dollars = dollars
					.checked_add(
						&props_cost
							.video
							.checked_mul(&<MultiBalanceOf<T>>::from(props.video))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

			room.props.picture =
				room.props.picture.checked_add(props.picture).ok_or(Error::<T>::Overflow)?;
			room.props.text =
				room.props.text.checked_add(props.text).ok_or(Error::<T>::Overflow)?;
			room.props.video =
				room.props.video.checked_add(props.video).ok_or(Error::<T>::Overflow)?;
			room.total_balances =
				room.total_balances.checked_add(&dollars.clone()).ok_or(Error::<T>::Overflow)?;

			room = Self::update_user_consume_amount(&who, room, dollars)?;
			AllRoom::<T>::insert(group_id, room);

			let mut person = <PurchaseSummary<T>>::get(who.clone());
			person.props.picture =
				person.props.picture.checked_add(props.picture).ok_or(Error::<T>::Overflow)?;
			person.props.text =
				person.props.text.checked_add(props.text).ok_or(Error::<T>::Overflow)?;
			person.props.video =
				person.props.video.checked_add(props.video).ok_or(Error::<T>::Overflow)?;
			person.cost = person.cost.checked_add(&dollars.clone()).ok_or(Error::<T>::Overflow)?;
			<PurchaseSummary<T>>::insert(who.clone(), person);

			T::MultiCurrency::withdraw(T::GetNativeCurrencyId::get(), &who, dollars)?;

			// Self::get_like(&who, dollars)?;
			Self::deposit_event(Event::BuyProps(who));
			Ok(())
		}

		/// Users buy audio in the room.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn buy_audio_in_room(
			origin: OriginFor<T>,
			group_id: RoomId,
			audio: Audio,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			let mut dollars = <MultiBalanceOf<T>>::from(0u32);
			ensure!(
				audio.ten_seconds != 0u32 || audio.thirty_seconds != 0u32 || audio.minutes != 0u32,
				Error::<T>::BuyNothing
			);
			let audio_cost = <AudioPayment<T>>::get();

			if audio.ten_seconds > 0u32 {
				dollars = audio_cost
					.ten_seconds
					.checked_mul(&<MultiBalanceOf<T>>::from(audio.ten_seconds))
					.ok_or(Error::<T>::Overflow)?;
			}

			if audio.thirty_seconds > 0u32 {
				dollars = dollars
					.checked_add(
						&audio_cost
							.thirty_seconds
							.checked_mul(&<MultiBalanceOf<T>>::from(audio.thirty_seconds))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

			if audio.minutes > 0u32 {
				dollars = dollars
					.checked_add(
						&audio_cost
							.minutes
							.checked_mul(&<MultiBalanceOf<T>>::from(audio.minutes))
							.ok_or(Error::<T>::Overflow)?,
					)
					.ok_or(Error::<T>::Overflow)?;
			}

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
			room = Self::update_user_consume_amount(&who, room, dollars)?;
			AllRoom::<T>::insert(group_id, room);

			let mut person = <PurchaseSummary<T>>::get(who.clone());
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
			<PurchaseSummary<T>>::insert(who.clone(), person);

			T::MultiCurrency::withdraw(T::GetNativeCurrencyId::get(), &who, dollars)?;

			// Self::get_like(&who, dollars)?;
			Self::deposit_event(Event::BuyAudio(who));
			Ok(())
		}

		/// Set the cost of creating the room
		///
		/// The Origin must be Root.
		#[pallet::weight(1500_000_000)]
		pub fn set_create_cost(
			origin: OriginFor<T>,
			max_members: GroupMaxMembers,
			amount: MultiBalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;

			CreatePayment::<T>::try_mutate(max_members, |h| -> DispatchResult {
				ensure!(amount != *h, Error::<T>::NotChange);
				*h = amount;
				Ok(())
			})?;

			Self::deposit_event(Event::SetCreateCost);
			Ok(())
		}

		/// Remove someone from a blacklist
		///
		/// The Origin must be RoomCouncil or RoomManager.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn remove_someone_from_blacklist(
			origin: OriginFor<T>,
			group_id: RoomId,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			T::RoomRootOrHalfCouncilOrigin::try_origin(origin)
				.map_err(|_| Error::<T>::BadOrigin)?;

			let who = T::Lookup::lookup(who)?;
			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);

			AllRoom::<T>::try_mutate_exists(group_id, |r| -> DispatchResult {
				let room = r.as_mut().take().ok_or(Error::<T>::RoomNotExists)?;

				if let Some(pos) = room.black_list.iter().position(|h| h == &who) {
					room.black_list.remove(pos);
				} else {
					return Err(Error::<T>::NotInBlackList)?
				}
				*r = Some(room.clone());
				Ok(())
			})?;

			Self::deposit_event(Event::RemoveSomeoneFromBlackList(who, group_id));
			Ok(())
		}

		/// Remove someone(not room owner or council members) in the room.
		///
		/// The Origin must be RoomCouncil or RoomManager.
		/// fixme Further refinement of the various votes is needed
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn remove_someone(
			origin: OriginFor<T>,
			group_id: RoomId,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			// For the time being, only the group master will execute it
			T::RoomRootOrHalfRoomCouncilOrSomeRoomCouncilOrigin::try_origin(origin.clone())
				.map_err(|_| Error::<T>::BadOrigin)?;

			let who = T::Lookup::lookup(who)?;

			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			ensure!(
				room.group_manager != Some(who.clone()) && !room.council.contains(&who),
				Error::<T>::RoomManager
			);
			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);

			let now = Self::now();

			if T::RoomRootOrigin::try_origin(origin).is_ok() {
				let until = now.saturating_sub(room.last_remove_someone_block);
				let interval = Self::remove_interval(Self::members_convert_type(room.max_members)?);
				ensure!(until > interval, Error::<T>::IsNotRemoveTime);

				room.last_remove_someone_block = now;
			}

			room.black_list.push(who.clone());
			room = Self::remove_someone_in_room(who.clone(), room);
			AllRoom::<T>::insert(group_id, room);

			Self::deposit_event(Event::Kicked(who.clone(), group_id));
			Ok(())
		}

		/// Request dismissal of the room
		///
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn ask_for_disband_room(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// fixme
			// ensure!(!Self::is_voting() && !(Self::vote_passed_and_pending_disband(group_id) == Ok(true)), Error::<T>::Disbanding);
			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			let now = Self::now();

			// Groups cannot be dissolved for a period of time after they are created
			ensure!(
				now.saturating_sub(room.create_block) > T::ProtectedDuration::get(),
				Error::<T>::InProtectedDuration
			);

			// If there's ever been a request to dissolve
			let until = now.saturating_sub(room.disband_vote_end_block);
			let interval = Self::disband_interval(Self::members_convert_type(room.max_members)?);
			ensure!(until > interval, Error::<T>::IsNotAskForDisbandTime);

			ensure!(!(Self::is_voting(&room)), Error::<T>::IsVoting);

			// Those who ask for dissolution will have to deduct some of the amount to the Treasury.
			let disband_payment = Percent::from_percent(10) * room.create_payment;
			let to = Self::treasury_id();
			T::MultiCurrency::transfer(T::GetNativeCurrencyId::get(), &who, &to, disband_payment)?;

			room.disband_vote = DisbandVote::default();
			room.disband_vote_end_block = Self::now() + T::VoteExpire::get();
			room.disband_vote.approve_man.insert(who.clone());
			let user_consume_amount = Self::get_user_consume_amount(&who, &room);
			room.disband_vote.approve_total_amount = room
				.disband_vote
				.approve_total_amount
				.checked_add(&user_consume_amount)
				.ok_or(Error::<T>::Overflow)?;
			// It is possible that this person went to cast a vote and passed
			room = Self::judge_vote_and_update_room(room)?;
			AllRoom::<T>::insert(group_id, room);

			Self::deposit_event(Event::AskForDisband(who.clone(), group_id));
			Ok(())
		}

		/// Set the price of the audio
		///
		/// The Origin must be Root.
		#[pallet::weight(1500_000_000)]
		pub fn set_audio_price(
			origin: OriginFor<T>,
			cost: AudioPrice<MultiBalanceOf<T>>,
		) -> DispatchResult {
			ensure_root(origin)?;

			<AudioPayment<T>>::put(cost);
			Self::deposit_event(Event::SetAudioPrice);
			Ok(())
		}

		/// Set the price of the props.
		///
		/// The Origin must be Root.
		#[pallet::weight(1500_000_000)]
		pub fn set_props_price(
			origin: OriginFor<T>,
			cost: PropsPrice<MultiBalanceOf<T>>,
		) -> DispatchResult {
			ensure_root(origin)?;

			<PropsPayment<T>>::put(cost);
			Self::deposit_event(Event::SetPropsPrice);
			Ok(())
		}

		/// Sets the interval between removes someone in the room.
		///
		/// The Origin must be Root.
		#[pallet::weight(1500_000_000)]
		pub fn set_remove_interval(
			origin: OriginFor<T>,
			max_members: GroupMaxMembers,
			interval: T::BlockNumber,
		) -> DispatchResult {
			ensure_root(origin)?;

			<RemoveInterval<T>>::try_mutate(max_members, |h| -> DispatchResult {
				ensure!(interval != *h, Error::<T>::NotChange);
				*h = interval;
				Ok(())
			})?;
			Self::deposit_event(Event::SetKickInterval);
			Ok(())
		}

		/// Set the interval at which the group is dismissed.
		#[pallet::weight(1500_000_000)]
		pub fn set_disband_interval(
			origin: OriginFor<T>,
			max_members: GroupMaxMembers,
			interval: T::BlockNumber,
		) -> DispatchResult {
			ensure_root(origin)?;

			<DisbandInterval<T>>::try_mutate(max_members, |h| -> DispatchResult {
				ensure!(interval != *h, Error::<T>::NotChange);
				*h = interval;
				Ok(())
			})?;

			Self::deposit_event(Event::SetDisbandInterval);
			Ok(())
		}

		/// Users vote for disbanding the room.
		///
		///
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn vote(origin: OriginFor<T>, group_id: RoomId, vote: ListenVote) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			// the consume amount should not be zero.
			let user_consume_amount = Self::get_user_consume_amount(&who, &room);
			if user_consume_amount == <MultiBalanceOf<T>>::from(0u32) {
				return Err(Error::<T>::ConsumeAmountIsZero)?
			}

			// The vote does not exist or is expired
			if !Self::is_voting(&room) {
				room = Self::remove_vote_info(room);
				AllRoom::<T>::insert(group_id, room);
				return Err(Error::<T>::NotVoting)?
			}

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

			room = Self::judge_vote_and_update_room(room)?;
			AllRoom::<T>::insert(group_id, room);

			Self::deposit_event(Event::DisbandVote(who.clone(), group_id));
			Ok(())
		}

		/// Users get their reward in disbanded rooms.
		///
		/// the reward status should be NotGet in rooms.
		///
		/// Claim all rooms at once
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn pay_out(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				<PurchaseSummary<T>>::contains_key(who.clone()) &&
					!<PurchaseSummary<T>>::get(who.clone()).rooms.is_empty(),
				Error::<T>::NotInAnyRoom
			);

			let rooms = <PurchaseSummary<T>>::get(who.clone()).rooms;
			let mut rooms_cp = rooms.clone();
			let mut amount = <MultiBalanceOf<T>>::from(0u32);

			for room in rooms.iter() {
				// You must be in a NotGet state to receive a reward
				if room.1 == RewardStatus::NotGet {
					let group_id = room.0;

					// The room information was deleted when the room was disbanded. If it still exists,
					// the room is not disbanded. No reward
					if !<AllRoom<T>>::contains_key(group_id) {
						let session_index = Self::get_session_index();
						let mut is_get = false;

						for i in 0..Self::depth() as usize {
							let cur_session = session_index.saturating_sub(i as u32);

							// Reward only for disbanded groups.
							if <InfoOfDisbandedRoom<T>>::contains_key(cur_session, group_id) {
								let mut info = <InfoOfDisbandedRoom<T>>::get(cur_session, group_id);
								info.already_get_count = info
									.already_get_count
									.checked_add(1u32)
									.ok_or(Error::<T>::Overflow)?;

								// Everyone's bonus has been calculated at the time of disbandment.
								let reward = info.per_man_reward;

								// Provided for use only by events
								amount = amount.saturating_add(reward);

								info.already_get_reward = info
									.already_get_reward
									.checked_add(&reward)
									.ok_or(Error::<T>::Overflow)?;
								if info.already_get_count == info.total_person {
									<ListenersOfRoom<T>>::remove(group_id);
								} else {
									<ListenersOfRoom<T>>::mutate(group_id, |h| h.remove(&who));
								}

								<InfoOfDisbandedRoom<T>>::insert(cur_session, group_id, info);

								T::MultiCurrency::deposit(
									T::GetNativeCurrencyId::get(),
									&who,
									reward,
								)?;

								if let Some(pos) = rooms.iter().position(|h| h.0 == group_id) {
									rooms_cp.remove(pos);
									rooms_cp.insert(pos, (group_id, RewardStatus::Get));
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
								rooms_cp.remove(pos);
								rooms_cp.insert(pos, (group_id, RewardStatus::Expire));
							}
						}
					}
				}
			}

			<PurchaseSummary<T>>::mutate(who.clone(), |h| h.rooms = rooms_cp);

			if amount.is_zero() {
				return Err(Error::<T>::RewardAmountIsZero)?
			} else {
				Self::deposit_event(Event::Payout(who.clone(), amount));
			}

			Ok(())
		}

		/// Set the minimum amount for a person in a red envelope
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn set_redpack_min_amount(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
			min_amount: MultiBalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				MinRedPackAmount::<T>::get(currency_id) != min_amount,
				Error::<T>::AmountNotChange
			);
			MinRedPackAmount::<T>::insert(currency_id, min_amount);
			Self::deposit_event(Event::SetRedpackMinAmount(currency_id, min_amount));
			Ok(())
		}

		/// Users send red envelopes in the room.
		///
		///
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn send_redpacket_in_room(
			origin: OriginFor<T>,
			group_id: RoomId,
			currency_id: CurrencyIdOf<T>,
			lucky_man_number: u32,
			amount: MultiBalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(lucky_man_number != 0u32, Error::<T>::ParamIsZero);

			let _ = Self::room_exists_and_user_in_room(group_id, &who)?;

			// We dealt with the red envelopes when we disbanded the group.
			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);

			ensure!(
				MinRedPackAmount::<T>::get(currency_id)
					.checked_mul(&MultiBalanceOf::<T>::from(lucky_man_number))
					.ok_or(Error::<T>::Overflow)? <=
					amount,
				Error::<T>::AmountTooLow
			);
			let redpacket_id = <NextRedPacketId<T>>::get();

			let redpacket = RedPacket {
				id: redpacket_id,
				currency_id,
				boss: who.clone(),
				total: amount,
				lucky_man_number,
				already_get_man: BTreeSet::<T::AccountId>::default(),
				min_amount_of_per_man: MinRedPackAmount::<T>::get(currency_id),
				already_get_amount: MultiBalanceOf::<T>::from(0u32),
				end_time: Self::now()
					.checked_add(&T::RedPackExpire::get())
					.ok_or(Error::<T>::Overflow)?,
			};

			T::MultiCurrency::withdraw(currency_id, &who, amount)?;

			let now_id = redpacket_id.checked_add(1).ok_or(Error::<T>::Overflow)?;
			<NextRedPacketId<T>>::put(now_id);
			<RedPacketOfRoom<T>>::insert(group_id, redpacket_id, redpacket);

			Self::deposit_event(Event::SendRedPocket(group_id, redpacket_id, amount));
			Ok(())
		}

		/// Expired red packets obtained by the owner.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn give_back_expired_redpacket(
			origin: OriginFor<T>,
			group_id: RoomId,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let _ = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			Self::remove_redpacket_by_room_id(group_id, false)?;
			Self::deposit_event(Event::ReturnExpiredRedpacket(group_id));
			Ok(())
		}

		/// Users exit the room
		///
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn exit(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			let user = ensure_signed(origin)?;

			let mut room = Self::room_exists_and_user_in_room(group_id, &user)?;
			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			let number = room.now_members_number;
			let users_amount = room
				.total_balances
				.checked_sub(&room.group_manager_balances)
				.ok_or(Error::<T>::Overflow)?;

			if number > 1 {
				ensure!(
					room.group_manager != Some(user.clone()) && !room.council.contains(&user),
					Error::<T>::RoomManager
				);
				// If you quit halfway, you only get a quarter of the reward.
				let amount = users_amount /
					room.now_members_number.saturated_into::<MultiBalanceOf<T>>() /
					4u32.saturated_into::<MultiBalanceOf<T>>();
				T::MultiCurrency::deposit(T::GetNativeCurrencyId::get(), &user, amount)?;
				room.total_balances =
					room.total_balances.checked_sub(&amount).ok_or(Error::<T>::Overflow)?;
				room = Self::remove_someone_in_room(user.clone(), room);
				AllRoom::<T>::insert(group_id, room);
			} else {
				let amount = room.total_balances;
				T::MultiCurrency::deposit(T::GetNativeCurrencyId::get(), &user, amount)?;
				<AllRoom<T>>::remove(group_id);
				<ListenersOfRoom<T>>::remove(group_id);
				Self::remove_redpacket_by_room_id(group_id, true)?;
			}

			Self::deposit_event(Event::Exit(user, group_id));
			Ok(())
		}

		/// Disbanding the room.
		///
		/// The vote has been passed.
		///
		/// The Origin can be everyone.
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn disband_room(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_can_disband(group_id)?, Error::<T>::NotDisbanding);
			let room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			Self::do_disband(room)?;
			Self::deposit_event(Event::DisbandRoom(group_id, who));
			Ok(())
		}

		// /// Council Members reject disband the room.
		// #[pallet::weight(1500_000_000)]
		// #[transactional]
		// pub fn council_reject_disband(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
		// 	T::HalfRoomCouncilOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;
		//
		// 	let disband_rooms = <PendingDisbandRooms<T>>::get();
		// 	let mut room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
		// 	if let Some(disband_time) = disband_rooms.get(&group_id.into()) {
		// 		/// before the room is disband.
		// 		if Self::now() <= *disband_time {
		// 			<PendingDisbandRooms<T>>::mutate(|h| h.remove(&group_id.into()));
		// 			room = Self::remove_vote_info(room);
		// 			AllRoom::<T>::insert(group_id, room);
		// 		} else {
		// 			return Err(Error::<T>::Disbanding)?
		// 		}
		// 	} else {
		// 		return Err(Error::<T>::NotDisbanding)?
		// 	}
		//
		// 	Self::deposit_event(Event::CouncilRejectDisband(group_id));
		// 	Ok(())
		// }

		/// Multi account to help get red packets
		#[pallet::weight(1500_000_000)]
		#[transactional]
		pub fn get_redpacket_in_room(
			origin: OriginFor<T>,
			info: Vec<(<T::Lookup as StaticLookup>::Source, MultiBalanceOf<T>)>,
			group_id: RoomId,
			redpacket_id: u128,
		) -> DispatchResult {
			let mul = ensure_signed(origin)?;

			let mut members: Vec<<T::Lookup as StaticLookup>::Source> = vec![];
			for i in info.clone() {
				members.push(i.0)
			}
			Self::check_accounts(members)?;

			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigNotExists)?;
			ensure!(mul == multisig_id, Error::<T>::NotMultisigId);

			let mut redpacket = <RedPacketOfRoom<T>>::get(group_id, redpacket_id)
				.ok_or(Error::<T>::RedPacketNotExists)?;
			if redpacket.end_time < Self::now() {
				Self::remove_redpacket(group_id, &redpacket)?;
				return Err(Error::<T>::Expire)?
			}

			let mut total_amount = <MultiBalanceOf<T>>::from(0u32);

			for (who, amount) in info {
				let who = T::Lookup::lookup(who)?;

				if Self::room_exists_and_user_in_room(group_id, &who).is_err() {
					continue
				}

				// ensure!(amount >= T::RedPacketMinAmount::get(), Error::<T>::AmountTooLow);
				if amount < redpacket.min_amount_of_per_man {
					continue
				}

				if redpacket.total.clone().saturating_sub(redpacket.already_get_amount) < amount {
					continue
				}

				if redpacket.lucky_man_number <= redpacket.already_get_man.len() as u32 {
					break
				}

				if redpacket.already_get_man.contains(&who) == true {
					continue
				}

				T::MultiCurrency::deposit(redpacket.currency_id.clone(), &who, amount)?;

				redpacket.already_get_man.insert(who.clone());
				redpacket.already_get_amount = redpacket
					.already_get_amount
					.checked_add(&amount)
					.ok_or(Error::<T>::Overflow)?;

				if redpacket.already_get_man.len() == (redpacket.lucky_man_number as usize) {
					Self::remove_redpacket(group_id, &redpacket)?;
				} else {
					<RedPacketOfRoom<T>>::insert(group_id, redpacket_id, redpacket.clone());
				}

				if redpacket.already_get_amount == redpacket.total {
					<RedPacketOfRoom<T>>::remove(group_id, redpacket_id);
				}

				total_amount = total_amount.saturating_add(amount);
			}

			Self::deposit_event(Event::GetRedPocket(group_id, redpacket_id, total_amount));
			Ok(())
		}
	}

	// #[pallet::hooks]
	// impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
	// 	fn on_initialize(_: T::BlockNumber) -> Weight {
	// 		Self::remove_expire_disband_info();
	// 		<NowSessionIndex<T>>::put(Self::get_session_index());
	// 		0
	// 	}
	// }

	#[pallet::error]
	pub enum Error<T> {
		/// The Vec is empty.
		VecEmpty,
		/// The room is not exists.
		RoomNotExists,
		/// There was no one in the room.
		RoomMembersIsZero,
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
		VecTooLarge,
		BalanceIsNotZero,
		MultisigNotExists,
		AmountShouldDifferent,
		MembersNumberToMax,
		DivByZero,
		NotRewardTime,
		/// You didn't buy anything
		ConsumeAmountIsZero,
		RoomMembersToMax,
		RoomMaxNotDiff,
		InBlackList,
		NotInBlackList,
		InProtectedDuration,
		ThreshouldOrLenErr,
		NotUnique,
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
		AmountNotChange,
		MemberIsEmpty,
		NotChange,
		RoomManagerNotExists,
		MemberAlreadyInCouncil,
		NotRoomMember,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SetMultisig,
		AirDroped(u8, Vec<T::AccountId>),
		CreatedRoom(T::AccountId, RoomId),
		Invited(T::AccountId, T::AccountId),
		IntoRoom(T::AccountId, T::AccountId, RoomId),
		RejectedInvite(T::AccountId, RoomId),
		ChangedPermission(T::AccountId, RoomId),
		BuyProps(T::AccountId),
		BuyAudio(T::AccountId),
		Kicked(T::AccountId, RoomId),
		AskForDisband(T::AccountId, RoomId),
		DisbandVote(T::AccountId, RoomId),
		Payout(T::AccountId, MultiBalanceOf<T>),
		SendRedPocket(RoomId, u128, MultiBalanceOf<T>),
		GetRedPocket(RoomId, u128, MultiBalanceOf<T>),
		JoinCostChanged(RoomId, MultiBalanceOf<T>),
		SetPropsPrice,
		SetAudioPrice,
		SetDisbandInterval,
		SetKickInterval,
		SetCreateCost,
		SetServerId(T::AccountId),
		Exit(T::AccountId, RoomId),
		ManagerGetReward(T::AccountId, MultiBalanceOf<T>),
		SetMaxNumberOfRoomMembers(T::AccountId, u32),
		RemoveSomeoneFromBlackList(T::AccountId, RoomId),
		SetRoomPrivacy(RoomId, bool),
		DisbandRoom(RoomId, T::AccountId),
		CouncilRejectDisband(RoomId),
		ReturnExpiredRedpacket(RoomId),
		SetRedpackMinAmount(CurrencyIdOf<T>, MultiBalanceOf<T>),
		SetCouncilMembers(RoomId),
		AddCouncilMember(RoomId, T::AccountId),
		RemoveCouncilMember(RoomId, T::AccountId),
	}

	impl<T: Config> Pallet<T> {
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

		// fn u128_convert_balance(
		// 	amount: Balance,
		// ) -> result::Result<MultiBalanceOf<T>, DispatchError> {
		// 	let balances = <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(amount)
		// 		.map_err(|_| Error::<T>::NumberCanNotConvert)?;
		// 	Ok(balances)
		// }

		fn block_convert_balance(num: T::BlockNumber) -> MultiBalanceOf<T> {
			num.saturated_into::<u32>().saturated_into::<MultiBalanceOf<T>>()
		}

		pub fn treasury_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		pub fn now() -> T::BlockNumber {
			<system::Pallet<T>>::block_number()
		}

		fn add_new_user_info(yourself: &T::AccountId, group_id: RoomId) {
			<ListenersOfRoom<T>>::mutate(group_id, |h| h.insert(yourself.clone()));
			<PurchaseSummary<T>>::mutate(yourself.clone(), |h| {
				h.rooms.push((group_id, RewardStatus::default()))
			});
		}

		fn get_room_consume_amount(room: RoomInfoOf<T>) -> MultiBalanceOf<T> {
			let mut consume_total_amount = <MultiBalanceOf<T>>::from(0u32);
			for (_, amount) in room.consume.clone().iter() {
				consume_total_amount = consume_total_amount.saturating_add(*amount);
			}

			consume_total_amount
		}

		fn vote_passed_and_pending_disband(
			group_id: RoomId,
		) -> result::Result<bool, DispatchError> {
			<AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			if <PendingDisbandRooms<T>>::get().get(&group_id.into()).is_some() {
				return Ok(true)
			}
			Ok(false)
		}

		fn get_reward_info(
			room_info: &RoomInfoOf<T>,
		) -> result::Result<
			(MultiBalanceOf<T>, MultiBalanceOf<T>, MultiBalanceOf<T>, T::BlockNumber),
			DispatchError,
		> {
			let mut last_block = room_info.last_block_of_get_the_reward.clone();
			let time = Self::now().saturating_sub(last_block);
			let mut duration_num =
				time.checked_div(&T::RewardDuration::get()).ok_or(Error::<T>::DivByZero)?;
			let real_duration_num = duration_num.clone();
			if duration_num.is_zero() {
				return Err(Error::<T>::NotRewardTime)?
			} else {
				duration_num = duration_num.min(T::BlockNumber::from(5u32));
				let total_reward = Self::get_room_consume_amount(room_info.clone())
					.checked_mul(
						&Self::block_convert_balance(duration_num)
							.saturated_into::<MultiBalanceOf<T>>(),
					)
					.ok_or(Error::<T>::Overflow)?;

				ensure!(!total_reward.is_zero(), Error::<T>::RewardAmountIsZero);
				let manager_proportion_amount = T::ManagerProportion::get() *
					total_reward.saturated_into::<MultiBalanceOf<T>>();
				let room_proportion_amount =
					T::RoomProportion::get() * total_reward.saturated_into::<MultiBalanceOf<T>>();
				last_block =
					last_block.saturating_add(real_duration_num * T::RewardDuration::get());
				return Ok((
					total_reward,
					manager_proportion_amount,
					room_proportion_amount,
					last_block,
				))
			}
		}

		fn room_exists_and_user_in_room(
			group_id: RoomId,
			who: &T::AccountId,
		) -> result::Result<RoomInfoOf<T>, DispatchError> {
			let room = AllRoom::<T>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let listeners = <ListenersOfRoom<T>>::get(group_id);
			if listeners.len() == 0 {
				return Err(Error::<T>::RoomMembersIsZero)?
			}
			if listeners.contains(&who) {
				Ok(room)
			} else {
				Err(Error::<T>::NotInRoom)?
			}
		}

		fn join_do(
			inviter: &T::AccountId,
			invitee: &T::AccountId,
			group_id: RoomId,
		) -> DispatchResult {
			AllRoom::<T>::try_mutate_exists(group_id, |room| -> DispatchResult {
				let mut room_info = room.take().ok_or(Error::<T>::RoomNotExists)?;
				let join_cost = room_info.join_cost;

				// Inviter pay for group entry.
				if !join_cost.is_zero() {
					T::MultiCurrency::withdraw(T::GetNativeCurrencyId::get(), &inviter, join_cost)?;
					room_info = Self::pay_for(join_cost, room_info)?;
				}

				room_info.now_members_number =
					room_info.now_members_number.checked_add(1u32).ok_or(Error::<T>::Overflow)?;

				Self::add_new_user_info(&invitee, group_id);
				*room = Some(room_info);
				Ok(())
			})
		}

		fn is_unique(members: Vec<T::AccountId>) -> bool {
			let members_cp = members.clone();
			for who in members {
				let mut index = 0u32;
				for member in members_cp.clone() {
					if member == who {
						index += 1;
					}
					if index > 1 {
						return false
					}
				}
			}
			true
		}

		fn pay_for(
			join_cost: MultiBalanceOf<T>,
			mut room_info: RoomInfoOf<T>,
		) -> result::Result<RoomInfoOf<T>, DispatchError> {
			let payment_manager_now = Percent::from_percent(5) * join_cost;
			let payment_manager_later = Percent::from_percent(5) * join_cost;
			let payment_room_later = Percent::from_percent(50) * join_cost;
			let payment_treasury = join_cost
				.saturating_sub(payment_manager_now)
				.saturating_sub(payment_manager_later)
				.saturating_sub(payment_room_later);

			room_info.total_balances = payment_room_later
				.checked_add(&room_info.total_balances)
				.ok_or(Error::<T>::Overflow)?;
			room_info.total_balances = payment_manager_later
				.checked_add(&room_info.total_balances)
				.ok_or(Error::<T>::Overflow)?;

			room_info.group_manager_balances = payment_manager_later
				.checked_add(&room_info.group_manager_balances)
				.ok_or(Error::<T>::Overflow)?;

			let treasury_id = Self::treasury_id();

			match room_info.group_manager.clone() {
				None => {
					T::MultiCurrency::deposit(
						T::GetNativeCurrencyId::get(),
						&treasury_id,
						payment_manager_now,
					)?;
				},
				Some(x) => {
					T::MultiCurrency::deposit(
						T::GetNativeCurrencyId::get(),
						&x,
						payment_manager_now,
					)?;
				},
			};

			T::MultiCurrency::deposit(
				T::GetNativeCurrencyId::get(),
				&treasury_id,
				payment_treasury,
			)?;
			Ok(room_info)
		}

		///
		/// Note: Spending amounts these involve voting, so it's possible that the group will be disbanded
		fn update_user_consume_amount(
			who: &T::AccountId,
			mut room_info: RoomInfoOf<T>,
			amount: MultiBalanceOf<T>,
		) -> result::Result<RoomInfoOf<T>, DispatchError> {
			let new_user_consume: (T::AccountId, MultiBalanceOf<T>);
			if let Some(pos) = room_info.consume.clone().iter().position(|h| h.0 == who.clone()) {
				let old_user_consume = room_info.consume.remove(pos);
				new_user_consume = (
					old_user_consume.0,
					old_user_consume.1.checked_add(&amount).ok_or(Error::<T>::Overflow)?,
				);
			} else {
				new_user_consume = (who.clone(), amount);
			}

			// Reorder the consumption queue
			{
				let mut index = 0;
				for per_consume in room_info.consume.iter() {
					if per_consume.1 < new_user_consume.1 {
						break
					}
					index += 1;
				}
				room_info.consume.insert(index, new_user_consume.clone());
			}

			if Self::is_voting(&room_info) {
				let mut is_in_vote = false;
				let disband_vote_info = room_info.disband_vote.clone();
				if disband_vote_info.approve_man.get(&who).is_some() {
					room_info.disband_vote.approve_total_amount = room_info
						.disband_vote
						.approve_total_amount
						.checked_add(&amount)
						.ok_or(Error::<T>::Overflow)?;
					is_in_vote = true;
				}
				if disband_vote_info.reject_man.get(&who).is_some() {
					room_info.disband_vote.reject_total_amount = room_info
						.disband_vote
						.reject_total_amount
						.checked_add(&amount)
						.ok_or(Error::<T>::Overflow)?;
					is_in_vote = true;
				}
				if is_in_vote {
					room_info = Self::judge_vote_and_update_room(room_info)?;
				}
			}

			Ok(room_info)
		}

		fn is_voting(room: &RoomInfoOf<T>) -> bool {
			if (Self::now().saturating_sub(room.disband_vote_end_block).is_zero()) &&
				!(Self::vote_passed_and_pending_disband(room.group_id) == Ok(true))
			{
				return true
			}
			false
		}

		fn judge_vote_and_update_room(
			mut room: RoomInfoOf<T>,
		) -> result::Result<RoomInfoOf<T>, DispatchError> {
			let group_id = room.group_id;
			let now = Self::now();
			let vote_result = Self::is_vote_end(room.clone());

			if vote_result.0 == END {
				if vote_result.1 == PASS {
					// The cost is 0. Dismiss immediately
					if vote_result.2 == <MultiBalanceOf<T>>::from(0u32) {
						Self::do_disband(room.clone())?;
					} else {
						room.disband_vote_end_block = now;
						<PendingDisbandRooms<T>>::mutate(|h| {
							h.insert(group_id.into(), now + T::DelayDisbandDuration::get())
						});
					}
				} else {
					room = Self::remove_vote_info(room.clone());
				}
			}
			Ok(room)
		}

		fn is_vote_end(room_info: RoomInfoOf<T>) -> (bool, bool, MultiBalanceOf<T>) {
			let end_time = room_info.disband_vote_end_block.clone();
			let approve_total_amount = room_info.disband_vote.approve_total_amount.clone();
			let reject_total_amount = room_info.disband_vote.reject_total_amount.clone();
			let consume_total_amount = Self::get_room_consume_amount(room_info.clone());
			if approve_total_amount * <MultiBalanceOf<T>>::from(2u32) >= consume_total_amount ||
				reject_total_amount * <MultiBalanceOf<T>>::from(2u32) >= consume_total_amount
			{
				if approve_total_amount >= reject_total_amount {
					(END, PASS, consume_total_amount)
				} else {
					(END, NOT_PASS, consume_total_amount)
				}
			} else {
				if end_time >= Self::now() {
					(NOT_END, NOT_PASS, consume_total_amount)
				} else {
					(END, NOT_PASS, consume_total_amount)
				}
			}
		}

		fn do_disband(room: RoomInfoOf<T>) -> DispatchResult {
			let group_id = room.group_id;
			Self::remove_redpacket_by_room_id(group_id, true)?;
			let total_reward = room.total_balances.clone();
			let manager_reward = match room.group_manager.clone() {
				None => MultiBalanceOf::<T>::from(0u32),
				Some(x) => {
					let amount = room.group_manager_balances.clone();
					T::MultiCurrency::deposit(T::GetNativeCurrencyId::get(), &x, amount)?;
					amount
				},
			};
			// room.group_manager_balances.clone();

			let listener_reward = total_reward.clone() - manager_reward.clone();
			let session_index = Self::get_session_index();
			let per_man_reward =
				listener_reward.clone() / <MultiBalanceOf<T>>::from(room.now_members_number);
			let room_rewad_info = RoomRewardInfo {
				total_person: room.now_members_number.clone(),
				already_get_count: 0u32,
				total_reward: listener_reward.clone(),
				already_get_reward: <MultiBalanceOf<T>>::from(0u32),
				per_man_reward: per_man_reward.clone(),
			};

			<InfoOfDisbandedRoom<T>>::insert(session_index, group_id, room_rewad_info);
			<ListenersOfRoom<T>>::remove(group_id);
			<AllRoom<T>>::remove(group_id);
			<PendingDisbandRooms<T>>::mutate(|h| h.remove(&group_id.into()));
			T::CollectiveHandler::remove_room_collective_info(group_id.into())?;
			T::RoomTreasuryHandler::remove_room_treasury_info(group_id.into());
			Ok(())
		}

		fn remove_vote_info(mut room: RoomInfoOf<T>) -> RoomInfoOf<T> {
			room.disband_vote = <DisbandVote<BTreeSet<T::AccountId>, MultiBalanceOf<T>>>::default();
			let now = Self::now();
			if room.disband_vote_end_block > now {
				room.disband_vote_end_block = now;
			}
			room
		}

		fn get_session_index() -> SessionIndex {
			let now = Self::now().saturated_into::<SessionIndex>();
			let session_index = now / EPOCH_DURATION_IN_BLOCKS;
			session_index
		}

		fn remove_redpacket_by_room_id(room_id: RoomId, all: bool) -> DispatchResult {
			let redpackets = <RedPacketOfRoom<T>>::iter_prefix(room_id).collect::<Vec<_>>();
			let now = Self::now();

			if all {
				for redpacket in redpackets.iter() {
					Self::remove_redpacket(room_id, &redpacket.1)?;
				}
			} else {
				for redpacket in redpackets.iter() {
					if redpacket.1.end_time < now {
						Self::remove_redpacket(room_id, &redpacket.1)?;
					}
				}
			}
			Ok(())
		}

		fn remove_redpacket(
			room_id: RoomId,
			redpacket: &RedPacket<
				T::AccountId,
				BTreeSet<T::AccountId>,
				MultiBalanceOf<T>,
				T::BlockNumber,
				CurrencyIdOf<T>,
			>,
		) -> DispatchResult {
			let who = redpacket.boss.clone();
			let currency_id = redpacket.currency_id;
			let remain = redpacket.total.saturating_sub(redpacket.already_get_amount);
			let redpacket_id = redpacket.id;
			T::MultiCurrency::deposit(currency_id, &who, remain)?;
			<RedPacketOfRoom<T>>::remove(room_id, redpacket_id);
			Ok(())
		}

		fn remove_someone_in_room(who: T::AccountId, mut room: RoomInfoOf<T>) -> RoomInfoOf<T> {
			room.now_members_number = room.now_members_number.saturating_sub(1);
			room = Self::remove_consumer_info(room, who.clone());
			let mut listeners = <ListenersOfRoom<T>>::get(room.group_id);
			listeners.take(&who);
			// if room.group_manager == Some(who.clone()) {
			// 	room.group_manager = None;
			// }
			//
			// // todo If a member of council has a vote, his vote should be removed
			// if let Some(pos) = room.council.iter().position(|h| h == &who) {
			// 	room.council.swap_remove(pos);
			// }

			<ListenersOfRoom<T>>::insert(room.group_id.clone(), listeners);
			room
		}

		fn remove_consumer_info(mut room: RoomInfoOf<T>, who: T::AccountId) -> RoomInfoOf<T> {
			// todo If there is a vote to dissolve the group, that vote should be removed
			if let Some(pos) = room.consume.iter().position(|h| h.0 == who.clone()) {
				room.consume.remove(pos);
			}

			room
		}

		// fn get_like(who: &T::AccountId, amount: MultiBalanceOf<T>) -> DispatchResult {
		// 	let mul_amount =
		// 		amount.saturated_into::<Balance>().saturated_into::<MultiBalanceOf<T>>();
		// 	T::MultiCurrency::deposit(T::GetLikeCurrencyId::get(), who, mul_amount)?;
		// 	Ok(())
		// }

		fn get_user_consume_amount(who: &T::AccountId, room: &RoomInfoOf<T>) -> MultiBalanceOf<T> {
			let mut room = room.clone();
			if let Some(pos) = room.consume.clone().iter().position(|h| &h.0 == who) {
				let consume = room.consume.remove(pos);
				return consume.1
			} else {
				return <MultiBalanceOf<T>>::from(0u32)
			}
		}

		pub fn is_can_disband(group_id: RoomId) -> result::Result<bool, DispatchError> {
			let _ = AllRoom::<T>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let disband_rooms = <PendingDisbandRooms<T>>::get();
			if let Some(disband_time) = disband_rooms.get(&group_id.into()) {
				if Self::now() >= *disband_time {
					return Ok(true)
				}
			}
			Ok(false)
		}

		// fixme
		// /// Delete information about rooms that have been dismissed
		// fn remove_expire_disband_info() -> DispatchResult {
		// 	let mut index: u32;
		// 	let mut info: Vec<(RoomId, RoomRewardInfo<MultiBalanceOf<T>>)>;
		//
		// 	let last_session_index = Self::last_session_index();
		// 	let now_session = Self::get_session_index();
		//
		// 	if now_session - last_session_index <= Self::depth() {
		// 		return
		// 	}
		//
		// 	index = last_session_index;
		// 	info = <InfoOfDisbandedRoom<T>>::iter_prefix(last_session_index).collect::<Vec<_>>();
		//
		// 	if info.len() == 0 {
		// 		let next = last_session_index.saturating_add(1);
		// 		let now_session = Self::get_session_index();
		// 		if now_session - next <= Self::depth() {
		// 			return
		// 		}
		// 		info = <InfoOfDisbandedRoom<T>>::iter_prefix(next).collect::<Vec<_>>();
		// 		index = next;
		// 	}
		//
		// 	info.truncate(200);
		// 	for i in info.iter() {
		// 		let group_id = i.0;
		// 		<ListenersOfRoom<T>>::remove(group_id);
		// 		let disband_room_info = <InfoOfDisbandedRoom<T>>::get(index, group_id);
		// 		let remain_reward =
		// 			disband_room_info.total_reward - disband_room_info.already_get_reward;
		// 		let teasury_id = Self::treasury_id();
		// 		T::MultiCurrency::deposit(
		// 			T::GetNativeCurrencyId::get(),
		// 			&teasury_id,
		// 			remain_reward,
		// 		)?;
		// 		<InfoOfDisbandedRoom<T>>::remove(index, group_id);
		// 	}
		// 	<LastSessionIndex<T>>::put(index);
		// 	Ok(())
		// }

		fn members_convert_type(num: u32) -> result::Result<GroupMaxMembers, DispatchError> {
			match num {
				0 => return Err(Error::<T>::MemberIsEmpty)?,
				1..=10 => Ok(GroupMaxMembers::Ten),
				11..=100 => Ok(GroupMaxMembers::Hundred),
				101..=500 => Ok(GroupMaxMembers::FiveHundred),
				501..=10000 => Ok(GroupMaxMembers::TenThousand),
				_ => Ok(GroupMaxMembers::NoLimit),
			}
		}
	}

	impl<T: Config> ListenHandler<RoomId, T::AccountId, DispatchError, u128> for Pallet<T> {
		fn get_room_council(
			room_id: RoomId,
		) -> Result<Vec<<T as frame_system::Config>::AccountId>, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = <AllRoom<T>>::get(room_id).ok_or(Error::<T>::RoomNotExists)?;
			Ok(room_info.council)
		}

		fn get_prime(
			room_id: RoomId,
		) -> Result<Option<<T as frame_system::Config>::AccountId>, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = <AllRoom<T>>::get(room_id).ok_or(Error::<T>::RoomNotExists)?;
			let prime = room_info.prime;
			Ok(prime)
		}

		fn get_root(
			room_id: RoomId,
		) -> Result<<T as frame_system::Config>::AccountId, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = <AllRoom<T>>::get(room_id).ok_or(Error::<T>::RoomNotExists)?;
			match room_info.group_manager {
				None => return Err(Error::<T>::RoomManagerNotExists)?,
				Some(x) => Ok(x),
			}
		}

		fn get_room_free_amount(room_id: RoomId) -> u128 {
			if let Some(room) = <AllRoom<T>>::get(room_id) {
				let disband_rooms = <PendingDisbandRooms<T>>::get();
				if let Some(disband_time) = disband_rooms.get(&room_id.into()) {
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

		fn sub_room_free_amount(room_id: RoomId, amount: u128) -> Result<(), DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let mut room = <AllRoom<T>>::get(room_id).ok_or(Error::<T>::RoomNotExists)?;
			if room.total_balances.saturating_sub(room.group_manager_balances) >=
				amount.saturated_into::<MultiBalanceOf<T>>()
			{
				room.total_balances =
					room.total_balances - amount.saturated_into::<MultiBalanceOf<T>>();
				<AllRoom<T>>::insert(room_id, room);
			} else {
				return Err(Error::<T>::RoomFreeAmountTooLow)?
			}
			Ok(())
		}
	}
}
