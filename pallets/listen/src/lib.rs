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
pub mod room_id;

pub use crate::pallet::*;
use codec::FullCodec;
use crate::primitives::{
	vote, AllProps, Audio, AudioPrice, DisbandVote, GroupInfo, GroupMaxMembers, ListenVote,
	PersonInfo, PropsPrice, RedPacket, RewardStatus, RoomRewardInfo, SessionIndex,
};
use codec::{Decode, Encode};
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
use orml_tokens::{self, BalanceLock};
use orml_traits::MultiCurrency;
use pallet_multisig;
use pallet_timestamp as timestamp;
pub use room_id::{AccountIdConversion as RoomIdConvertor, RoomId};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		AccountIdConversion, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Saturating,
		StaticLookup, Zero,
	},
	DispatchError, DispatchResult, Percent, RuntimeDebug, SaturatedConversion,
};
use sp_std::{
	cmp,
	collections::{btree_map::BTreeMap as BTMap, btree_set::BTreeSet},
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
		type RoomIdConvert: RoomIdConvertor<Self::AccountId>;
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
			/// set server id

			if let Some(server_id) = &self.server_id {
				ServerId::<T>::put(server_id);
			}

			/// set multisig id
			if self.multisig_members.len() >= 2 {
				let mut members = self.multisig_members.clone();
				members.sort();
				let threshould = 1u16;
				let multisig_id =
					<pallet_multisig::Module<T>>::multi_account_id(&members, threshould);
				<Multisig<T>>::put((members, threshould, multisig_id));
			}

			/// set the create room payment.
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

			/// set remove interval
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

			/// set disband interval
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
				len > 1 && threshould > 0u16 && threshould <= len as u16,
				Error::<T>::ThreshouldLenErr
			);

			match <ServerId<T>>::get() {
				Some(id) =>
					if id != server_id {
						return Err(Error::<T>::NotServerId)?
					},
				_ => return Err(Error::<T>::ServerIdNotExists)?,
			}

			let mut members = Self::check_accounts(members)?;
			members.sort();

			let multisig_id = <pallet_multisig::Module<T>>::multi_account_id(&members, threshould);
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
			<ServerId<T>>::put(&server_id);

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
			ensure!(who == multisig_id, Error::<T>::NotMultisigId);

			for user in members.iter() {
				if <AlreadyAirDropList<T>>::get().contains(&user) {
					continue
				}
				/// the account balances should be zero.
				if T::MultiCurrency::total_balance(T::GetNativeCurrencyId::get(), &user) !=
					Zero::zero()
				{
					continue
				}
				T::MultiCurrency::deposit(
					T::GetNativeCurrencyId::get(),
					&user,
					T::AirDropAmount::get(),
				)?;
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
		#[transactional]
		pub fn create_room(
			origin: OriginFor<T>,
			max_members: u32,
			group_type: Vec<u8>,
			join_cost: MultiBalanceOf<T>,
			is_private: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let create_payment = Self::create_cost(&Self::number_convert_type(max_members)?);

			/// the create cost transfers to the treasury
			let to = Self::treasury_id();
			T::MultiCurrency::transfer(T::GetNativeCurrencyId::get(), &who, &to, create_payment)?;
			let group_id = <NextGroupId<T>>::get();

			let group_info = GroupInfo {
				group_id,
				room_treasury_id: group_id.into_account(),
				create_payment,
				last_block_of_get_the_reward: Self::now(),
				group_manager: who.clone(),
				prime: None,
				max_members,
				group_type,
				join_cost,
				props: AllProps::default(),
				audio: Audio::default(),
				total_balances: <MultiBalanceOf<T>>::from(0u32),
				group_manager_balances: <MultiBalanceOf<T>>::from(0u32),
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
			Self::add_invitee_info(&who, group_id);
			<NextGroupId<T>>::try_mutate(|RoomId(h)| -> DispatchResult {
				*h = h.checked_add(1u64).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
			Self::deposit_event(Event::CreatedRoom(who, group_id));

			Ok(())
		}

		/// The room manager get his reward.
		/// You should have already created the room.
		/// Receive rewards for a fixed period of time
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn manager_get_reward(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			let mut room_info = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let group_manager = room_info.group_manager.clone();
			let mut last_block = room_info.last_block_of_get_the_reward.clone();
			let now = Self::now();
			let time = now.saturating_sub(last_block);
			let mut duration_num =
				time.checked_div(&T::RewardDuration::get()).ok_or(Error::<T>::DivByZero)?;

			if duration_num.is_zero() {
				return Err(Error::<T>::NotRewardTime)?
			} else {
				let max_num = T::BlockNumber::from(5u32);
				if duration_num > max_num {
					duration_num = max_num;
				}

				let consume_total_amount = Self::get_room_consume_amount(room_info.clone());
				let total_reward = consume_total_amount
					.checked_mul(
						&Self::block_convert_balance(duration_num)
							.saturated_into::<MultiBalanceOf<T>>(),
					)
					.ok_or(Error::<T>::Overflow)?;

				let manager_proportion_amount = T::ManagerProportion::get() *
					total_reward.saturated_into::<MultiBalanceOf<T>>();

				T::MultiCurrency::deposit(
					T::GetNativeCurrencyId::get(),
					&group_manager,
					manager_proportion_amount,
				)?;

				room_info.total_balances = room_info
					.total_balances
					.checked_sub(&manager_proportion_amount)
					.ok_or(Error::<T>::Overflow)?;
				let room_add_amount =
					T::RoomProportion::get() * total_reward.saturated_into::<MultiBalanceOf<T>>();
				room_info.total_balances = room_info
					.total_balances
					.clone()
					.checked_add(&room_add_amount)
					.ok_or(Error::<T>::Overflow)?;

				last_block = last_block.saturating_add(duration_num * T::RewardDuration::get());
				room_info.last_block_of_get_the_reward = last_block;

				<AllRoom<T>>::insert(group_id, room_info);

				Self::deposit_event(Event::ManagerGetReward(
					group_manager,
					manager_proportion_amount,
					room_add_amount,
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
			group_id: RoomId,
			join_cost: MultiBalanceOf<T>,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			let mut room_info = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			ensure!(room_info.join_cost != join_cost, Error::<T>::AmountShouldDifferent);
			room_info.join_cost = join_cost;
			<AllRoom<T>>::insert(group_id, room_info);

			Self::deposit_event(Event::JoinCostChanged(group_id, join_cost));
			Ok(())
		}

		/// The user joins the room.
		///
		/// If invitee is None, you go into the room by yourself. Otherwise, you invite people in.
		/// You can not invite yourself.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn into_room(
			origin: OriginFor<T>,
			group_id: RoomId,
			invitee: Option<<T::Lookup as StaticLookup>::Source>,
		) -> DispatchResult {
			let inviter = ensure_signed(origin)?;

			let mut invitee = match invitee {
				None => None,
				Some(x) => Some(T::Lookup::lookup(x)?),
			};

			let room_info = AllRoom::<T>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;

			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);

			// Private rooms must be invited by the manager.
			if room_info.is_private {
				ensure!(
					invitee.is_some() && inviter == room_info.group_manager,
					Error::<T>::PrivateRoom
				);
			}

			if let Some(x) = invitee.clone() {
				ensure!(x != inviter, Error::<T>::ShouldNotYourself);
			}

			// The guy who's gonna be in the group
			let man = invitee.unwrap_or_else(|| inviter.clone());

			/// people who into the room must be not in blacklist.
			let black_list = room_info.black_list;
			if let Some(pos) = black_list.iter().position(|h| h == &man) {
				return Err(Error::<T>::InBlackList)?
			};

			ensure!(
				room_info.max_members >= room_info.now_members_number.saturating_add(1u32),
				Error::<T>::MembersNumberToMax
			);

			ensure!(
				Self::room_exists_and_user_in_room(group_id, &man).is_err(),
				Error::<T>::InRoom
			);

			Self::join_do(&inviter, &man, group_id)?;

			Self::deposit_event(Event::IntoRoom(man, inviter, group_id));
			Ok(())
		}

		/// Set the privacy properties of the room
		///
		/// The Origin must be room manager.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn set_room_privacy(
			origin: OriginFor<T>,
			room_id: RoomId,
			is_private: bool,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			ensure!(!Self::vote_passed_and_pending_disband(room_id)?, Error::<T>::Disbanding);

			AllRoom::<T>::try_mutate_exists(room_id, |r| -> DispatchResult {
				let mut room = r.as_mut().take().ok_or(Error::<T>::RoomNotExists)?;
				ensure!(room.is_private != is_private, Error::<T>::PrivacyNotChange);
				room.is_private = is_private;
				*r = Some(room.clone());
				Ok(())
			})?;

			Self::deposit_event(Event::SetRoomPrivacy(room_id, is_private));
			Ok(())
		}

		/// Set the maximum number of people in the room
		///
		/// The Origin must be room manager.
		#[pallet::weight(10_000)]
		pub fn set_max_number_of_room_members(
			origin: OriginFor<T>,
			group_id: RoomId,
			new_max: u32,
		) -> DispatchResult {
			T::RoomRootOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			let mut room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let manager = room.group_manager.clone();
			let now_number = room.now_members_number;
			let now_max = room.max_members;
			ensure!(now_max != new_max, Error::<T>::RoomMaxNotDiff);

			// The value cannot be smaller than the current group size
			ensure!(now_number <= new_max, Error::<T>::RoomMembersToMax);

			let old_amount = Self::create_cost(&Self::number_convert_type(now_max)?);
			let new_amount = Self::create_cost(&Self::number_convert_type(new_max)?);

			let add_amount =
				new_amount.saturating_sub(old_amount).saturated_into::<MultiBalanceOf<T>>();
			if add_amount != Zero::zero() {
				/// transfers amount to the treasury.
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
		#[pallet::weight(10_000)]
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
			Self::update_user_consume(&who, &mut room, dollars)?;

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

			Self::get_like(&who, dollars);
			Self::deposit_event(Event::BuyProps(who));
			Ok(())
		}

		/// Users buy audio in the room.
		#[pallet::weight(10_000)]
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
			Self::update_user_consume(&who, &mut room, dollars)?;

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

			Self::get_like(&who, dollars);
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
		#[pallet::weight(10_000)]
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
				let mut room = r.as_mut().take().ok_or(Error::<T>::RoomNotExists)?;

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

		/// Remove someone from the room.
		///
		/// The Origin must be RoomCouncil or RoomManager.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn remove_someone(
			origin: OriginFor<T>,
			group_id: RoomId,
			who: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			T::RoomRootOrHalfRoomCouncilOrSomeRoomCouncilOrigin::try_origin(origin.clone())
				.map_err(|_| Error::<T>::BadOrigin)?;

			let who = T::Lookup::lookup(who)?;

			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			ensure!(room.group_manager != who.clone(), Error::<T>::RoomManager);
			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);

			let now = Self::now();

			if T::RoomRootOrigin::try_origin(origin).is_ok() {
				if room.last_remove_someone_block > T::BlockNumber::from(0u32) {
					let until = now.saturating_sub(room.last_remove_someone_block);
					let interval =
						Self::remove_interval(Self::number_convert_type(room.max_members)?);
					ensure!(until > interval, Error::<T>::IsNotRemoveTime);
				}
				room.last_remove_someone_block = now;
			}

			room.black_list.push(who.clone());
			Self::remove_someone_in_room(who.clone(), &mut room);

			Self::deposit_event(Event::Kicked(who.clone(), group_id));
			Ok(())
		}

		/// Request dismissal of the room
		///
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn ask_for_disband_room(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!Self::is_can_disband(group_id)?, Error::<T>::Disbanding);
			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			let create_block = room.create_block;
			let now = Self::now();

			// Groups cannot be dissolved for a period of time after they are created
			ensure!(
				now.saturating_sub(create_block) > T::ProtectedDuration::get(),
				Error::<T>::InProtectedDuration
			);

			// If there's ever been a request to dissolve
			if room.disband_vote_end_block > T::BlockNumber::from(0u32) {
				let until = now.saturating_sub(room.disband_vote_end_block);
				let interval = Self::disband_interval(Self::number_convert_type(room.max_members)?);
				ensure!(until > interval, Error::<T>::IsNotAskForDisbandTime);
			}

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
			Self::judge_vote_and_update_room(&mut room);

			Self::deposit_event(Event::AskForDisband(who.clone(), group_id));
			Ok(())
		}

		/// Set the price of the audio
		///
		/// The Origin must be Root.
		#[pallet::weight(10_000)]
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
		#[pallet::weight(10_000)]
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
		#[pallet::weight(10_000)]
		pub fn set_remove_interval(
			origin: OriginFor<T>,
			max_members: GroupMaxMembers,
			interval: T::BlockNumber,
		) -> DispatchResult {
			ensure_root(origin)?;

			<RemoveInterval<T>>::try_mutate(max_members, |h| -> DispatchResult {
				ensure!(interval != *h, Error::<T>::NotChange);
				Ok(())
			})?;
			Self::deposit_event(Event::SetKickInterval);
			Ok(())
		}

		/// Set the interval at which the group is dismissed.
		#[pallet::weight(10_000)]
		pub fn set_disband_interval(
			origin: OriginFor<T>,
			max_members: GroupMaxMembers,
			interval: T::BlockNumber,
		) -> DispatchResult {
			ensure_root(origin)?;

			<DisbandInterval<T>>::try_mutate(max_members, |h| -> DispatchResult {
				ensure!(interval != *h, Error::<T>::NotChange);
				Ok(())
			})?;

			Self::deposit_event(Event::SetDisbandInterval);
			Ok(())
		}

		/// Users vote for disbanding the room.
		///
		///
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn vote(origin: OriginFor<T>, group_id: RoomId, vote: ListenVote) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut room = Self::room_exists_and_user_in_room(group_id, &who)?;
			ensure!(!Self::vote_passed_and_pending_disband(group_id)?, Error::<T>::Disbanding);
			/// the consume amount should not be zero.
			let user_consume_amount = Self::get_user_consume_amount(&who, &room);
			if user_consume_amount == <MultiBalanceOf<T>>::from(0u32) {
				return Err(Error::<T>::ConsumeAmountIsZero)?
			}

			// The vote does not exist or is expired
			if !Self::is_voting(&room) {
				Self::remove_vote_info(room);
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

			Self::judge_vote_and_update_room(&mut room);

			Self::deposit_event(Event::DisbandVote(who.clone(), group_id));
			Ok(())
		}

		/// Users get their reward in disbanded rooms.
		///
		/// the reward status should be NotGet in rooms.
		///
		/// Claim all rooms at once
		#[pallet::weight(10_000)]
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
								);

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

		///
		/// Set the minimum amount for a person in a red envelope
		#[pallet::weight(10_000)]
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
		#[pallet::weight(10_000)]
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
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn give_back_expired_redpacket(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let _ = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			Self::remove_redpacket_by_room_id(group_id, false);
			Self::deposit_event(Event::ReturnExpiredRedpacket(group_id));
			Ok(())
		}

		/// Users exit the room
		///
		#[pallet::weight(10_000)]
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
				// If you quit halfway, you only get a quarter of the reward.
				let amount = users_amount /
					room.now_members_number.saturated_into::<MultiBalanceOf<T>>() /
					4u32.saturated_into::<MultiBalanceOf<T>>();
				T::MultiCurrency::deposit(T::GetNativeCurrencyId::get(), &user, amount)?;
				room.total_balances =
					room.total_balances.checked_sub(&amount).ok_or(Error::<T>::Overflow)?;
				Self::remove_someone_in_room(user.clone(), &mut room);
			} else {
				let amount = room.total_balances;
				T::MultiCurrency::deposit(T::GetNativeCurrencyId::get(), &user, amount)?;
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
		#[transactional]
		pub fn disband_room(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_can_disband(group_id)?, Error::<T>::NotDisbanding);
			let room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			Self::disband(room);
			Self::deposit_event(Event::DisbandRoom(group_id, who));
			Ok(())
		}

		/// Council Members reject disband the room.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn council_reject_disband(origin: OriginFor<T>, group_id: RoomId) -> DispatchResult {
			T::HalfRoomCouncilOrigin::try_origin(origin).map_err(|_| Error::<T>::BadOrigin)?;

			let disband_rooms = <PendingDisbandRooms<T>>::get();
			let room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			if let Some(disband_time) = disband_rooms.get(&group_id.into()) {
				/// before the room is disband.
				if Self::now() <= *disband_time {
					<PendingDisbandRooms<T>>::mutate(|h| h.remove(&group_id.into()));
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

		/// Multi account to help get red packets
		#[pallet::weight(10_000)]
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

			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigIdIsNone)?;
			ensure!(mul == multisig_id, Error::<T>::NotMultisigId);

			let mut redpacket = <RedPacketOfRoom<T>>::get(group_id, redpacket_id)
				.ok_or(Error::<T>::RedPacketNotExists)?;
			if redpacket.end_time < Self::now() {
				Self::remove_redpacket(group_id, &redpacket);
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

				T::MultiCurrency::deposit(redpacket.currency_id.clone(), &who, amount);

				redpacket.already_get_man.insert(who.clone());
				redpacket.already_get_amount = redpacket
					.already_get_amount
					.checked_add(&amount)
					.ok_or(Error::<T>::Overflow)?;

				if redpacket.already_get_man.len() == (redpacket.lucky_man_number as usize) {
					Self::remove_redpacket(group_id, &redpacket);
				} else {
					<RedPacketOfRoom<T>>::insert(group_id, redpacket_id, redpacket.clone());
				}

				if redpacket.already_get_amount == redpacket.total {
					<RedPacketOfRoom<T>>::remove(group_id, redpacket_id);
				}

				total_amount.saturating_add(amount);
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
		AmountNotChange,
		MemberIsEmpty,
		NotChange,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SetMultisig,
		AirDroped(T::AccountId),
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
		ManagerGetReward(T::AccountId, MultiBalanceOf<T>, MultiBalanceOf<T>),
		SetMaxNumberOfRoomMembers(T::AccountId, u32),
		RemoveSomeoneFromBlackList(T::AccountId, RoomId),
		SetRoomPrivacy(RoomId, bool),
		DisbandRoom(RoomId, T::AccountId),
		CouncilRejectDisband(RoomId),
		ReturnExpiredRedpacket(RoomId),
		SetRedpackMinAmount(CurrencyIdOf<T>, MultiBalanceOf<T>),
		ListenTest,
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

		fn u128_convert_balance(
			amount: Balance,
		) -> result::Result<MultiBalanceOf<T>, DispatchError> {
			let balances = <MultiBalanceOf<T> as TryFrom<Balance>>::try_from(amount)
				.map_err(|_| Error::<T>::NumberCanNotConvert)?;
			Ok(balances)
		}

		fn block_convert_balance(num: T::BlockNumber) -> MultiBalanceOf<T> {
			num.saturated_into::<u32>().saturated_into::<MultiBalanceOf<T>>()
		}

		pub fn treasury_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		pub fn now() -> T::BlockNumber {
			<system::Module<T>>::block_number()
		}

		fn add_invitee_info(yourself: &T::AccountId, group_id: RoomId) {
			<ListenersOfRoom<T>>::mutate(group_id, |h| h.insert(yourself.clone()));
			<PurchaseSummary<T>>::mutate(yourself.clone(), |h| {
				h.rooms.push((group_id, RewardStatus::default()))
			});
		}

		fn get_room_consume_amount(room: RoomInfoOf<T>) -> MultiBalanceOf<T> {
			let mut consume_total_amount = <MultiBalanceOf<T>>::from(0u32);
			for (account_id, amount) in room.consume.clone().iter() {
				consume_total_amount = consume_total_amount.saturating_add(*amount);
			}

			consume_total_amount
		}

		fn vote_passed_and_pending_disband(group_id: RoomId) -> result::Result<bool, DispatchError> {
			let _ = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let disband_rooms = <PendingDisbandRooms<T>>::get();
			if let Some(disband_time) = disband_rooms.get(&group_id.into()) {
				return Ok(true)
			}
			Ok(false)
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

		fn join_do(who: &T::AccountId, invitee: &T::AccountId, group_id: RoomId) -> DispatchResult {
			let room_info = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			let join_cost = room_info.join_cost;

			// Inviter pay for group entry.
			if join_cost != <MultiBalanceOf<T>>::from(0u32) {
				T::MultiCurrency::withdraw(T::GetNativeCurrencyId::get(), &who, join_cost)?;
				Self::pay_for(group_id, join_cost, room_info)?;
			}

			Self::add_invitee_info(&invitee, group_id);

			AllRoom::<T>::try_mutate_exists(group_id, |r| -> DispatchResult {
				let mut room_info = r.as_mut().take().ok_or(Error::<T>::RoomNotExists)?;
				room_info.now_members_number =
					room_info.now_members_number.checked_add(1u32).ok_or(Error::<T>::Overflow)?;
				*r = Some(room_info.clone());
				Ok(())
			})?;

			Ok(())
		}

		fn pay_for(
			group_id: RoomId,
			join_cost: MultiBalanceOf<T>,
			room_info: RoomInfoOf<T>,
		) -> DispatchResult {
			let payment_manager_now = Percent::from_percent(5) * join_cost;
			let payment_manager_later = Percent::from_percent(5) * join_cost;
			let payment_room_later = Percent::from_percent(50) * join_cost;
			let payment_treasury = Percent::from_percent(40) * join_cost;

			let mut room_info = room_info;
			room_info.total_balances = payment_room_later
				.checked_add(&room_info.total_balances)
				.ok_or(Error::<T>::Overflow)?;
			room_info.total_balances = payment_manager_later
				.checked_add(&room_info.total_balances)
				.ok_or(Error::<T>::Overflow)?;

			room_info.group_manager_balances = payment_manager_later
				.checked_add(&room_info.group_manager_balances)
				.ok_or(Error::<T>::Overflow)?;
			let group_manager = room_info.group_manager.clone();

			T::MultiCurrency::deposit(
				T::GetNativeCurrencyId::get(),
				&group_manager,
				payment_manager_now,
			)?;

			let teasury_id = Self::treasury_id();
			T::MultiCurrency::deposit(
				T::GetNativeCurrencyId::get(),
				&teasury_id,
				payment_treasury,
			)?;

			<AllRoom<T>>::insert(group_id, room_info);

			Ok(())
		}

		///
		/// Note: Spending amounts these involve voting, so it's possible that the group will be disbanded
		fn update_user_consume(
			who: &T::AccountId,
			room_info: &mut RoomInfoOf<T>,
			amount: MultiBalanceOf<T>,
		) -> DispatchResult {
			let group_id = room_info.group_id;
			let new_user_consume: (T::AccountId, MultiBalanceOf<T>);
			if let Some(pos) = room_info.consume.clone().iter().position(|h| h.0 == who.clone()) {
				let old_who_consume = room_info.consume.remove(pos);
				new_user_consume = (
					old_who_consume.0,
					old_who_consume.1.checked_add(&amount).ok_or(Error::<T>::Overflow)?,
				);
			} else {
				new_user_consume = (who.clone(), amount);
			}
			let mut index = 0;
			for per_consume in room_info.consume.iter() {
				if per_consume.1 < new_user_consume.1 {
					break
				}
				index += 1;
			}
			room_info.consume.insert(index, new_user_consume.clone());

			let mut consume = room_info.consume.clone();
			room_info.council = vec![];
			if consume.len() > T::CouncilMaxNumber::get() as usize {
				consume.split_off(T::CouncilMaxNumber::get() as usize);
				room_info.council = consume;
			} else {
				room_info.council = room_info.consume.clone();
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
					Self::judge_vote_and_update_room(room_info);
					return Ok(())
				}
			}
			<AllRoom<T>>::insert(group_id, room_info);

			Ok(())
		}

		fn is_voting(room: &RoomInfoOf<T>) -> bool {
			if !room.disband_vote_end_block.is_zero() &&
				(Self::now().saturating_sub(room.disband_vote_end_block).is_zero()) &&
				!(Self::vote_passed_and_pending_disband(room.group_id) == Ok(true))
			{
				return true
			}
			false
		}

		fn judge_vote_and_update_room(room: &mut RoomInfoOf<T>) {
			let group_id = room.group_id;
			let now = Self::now();
			let vote_result = Self::is_vote_end(room.clone());

			if vote_result.0 == End {
				if vote_result.1 == Pass {
					// The cost is 0. Dismiss immediately
					if vote_result.2 == <MultiBalanceOf<T>>::from(0u32) {
						Self::disband(room.clone());
					} else {
						room.disband_vote_end_block = now;
						<AllRoom<T>>::insert(group_id, room);
						<PendingDisbandRooms<T>>::mutate(|h| {
							h.insert(group_id.into(), now + T::DelayDisbandDuration::get())
						});
					}
				} else {
					Self::remove_vote_info(room.clone());
				}
				return
			}

			AllRoom::<T>::insert(group_id, room);
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

		fn disband(room: RoomInfoOf<T>) {
			let group_id = room.group_id;
			Self::remove_redpacket_by_room_id(group_id, true);
			let total_reward = room.total_balances.clone();
			let manager_reward = room.group_manager_balances.clone();
			T::MultiCurrency::deposit(
				T::GetNativeCurrencyId::get(),
				&room.group_manager,
				manager_reward,
			);
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
			T::CollectiveHandler::remove_room_collective_info(group_id.into());
			T::RoomTreasuryHandler::remove_room_treasury_info(group_id.into());
		}

		fn remove_vote_info(mut room: RoomInfoOf<T>) {
			room.disband_vote = <DisbandVote<BTreeSet<T::AccountId>, MultiBalanceOf<T>>>::default();
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

		fn remove_redpacket_by_room_id(room_id: RoomId, all: bool) {
			let redpackets = <RedPacketOfRoom<T>>::iter_prefix(room_id).collect::<Vec<_>>();
			let now = Self::now();

			if all {
				for redpacket in redpackets.iter() {
					Self::remove_redpacket(room_id, &redpacket.1);
				}
			} else {
				for redpacket in redpackets.iter() {
					if redpacket.1.end_time < now {
						Self::remove_redpacket(room_id, &redpacket.1);
					}
				}
			}
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
		) {
			let who = redpacket.boss.clone();
			let currency_id = redpacket.currency_id;
			let remain = redpacket.total.saturating_sub(redpacket.already_get_amount);
			let redpacket_id = redpacket.id;
			T::MultiCurrency::deposit(currency_id, &who, remain);
			<RedPacketOfRoom<T>>::remove(room_id, redpacket_id);
		}

		fn remove_someone_in_room(who: T::AccountId, room: &mut RoomInfoOf<T>) {
			room.now_members_number = room.now_members_number.saturating_sub(1);
			Self::remove_consumer_info(room, who.clone());
			let mut listeners = <ListenersOfRoom<T>>::get(room.group_id);
			listeners.take(&who);
			if room.group_manager == who {
				if room.consume.len() > 0 {
					let mut consume = room.consume.clone();
					room.group_manager = consume.remove(0).0;
				} else {
					room.group_manager = listeners.clone().iter().next().unwrap().clone();
				}
			}
			<ListenersOfRoom<T>>::insert(room.group_id.clone(), listeners);
			<AllRoom<T>>::insert(room.group_id, room);
		}

		fn remove_consumer_info(room: &mut RoomInfoOf<T>, who: T::AccountId) {
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
		}

		fn get_like(who: &T::AccountId, amount: MultiBalanceOf<T>) {
			let mul_amount =
				amount.saturated_into::<Balance>().saturated_into::<MultiBalanceOf<T>>();
			T::MultiCurrency::deposit(T::GetLikeCurrencyId::get(), who, mul_amount);
		}

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

		/// Delete information about rooms that have been dismissed
		fn remove_expire_disband_info() {
			let mut index: u32;
			let mut info: Vec<(RoomId, RoomRewardInfo<MultiBalanceOf<T>>)>;

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
				T::MultiCurrency::deposit(
					T::GetNativeCurrencyId::get(),
					&teasury_id,
					remain_reward,
				);
				<InfoOfDisbandedRoom<T>>::remove(index, group_id);
			}
			<LastSessionIndex<T>>::put(index);
		}

		fn number_convert_type(num: u32) -> result::Result<GroupMaxMembers, DispatchError> {
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
			let council_and_amount = room_info.council;
			let mut council = vec![];
			for i in council_and_amount.iter() {
				council.push(i.0.clone());
			}
			council.sort();
			Ok(council)
		}

		fn get_prime(
			room_id: RoomId,
		) -> Result<Option<<T as frame_system::Config>::AccountId>, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = <AllRoom<T>>::get(room_id).ok_or(Error::<T>::RoomNotExists)?;
			let prime = room_info.prime;
			Ok(prime)
		}

		fn get_root(room_id: RoomId) -> Result<<T as frame_system::Config>::AccountId, DispatchError> {
			ensure!(!Self::is_can_disband(room_id)?, Error::<T>::Disbanding);
			let room_info = <AllRoom<T>>::get(room_id).ok_or(Error::<T>::RoomNotExists)?;
			let root = room_info.group_manager;
			Ok(root)
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
