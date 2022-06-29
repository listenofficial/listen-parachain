// Copyright 2021-2022 LISTEN TEAM.
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

// Forked from https://github.com/open-web3-stack/open-runtime-module-library/tree/master/currencies.
// Most of this module uses code from the orml, but due to business differences, we made some feature additions.
// In this module, wo can create asset, set metadata and burn our tokens.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::Codec;
use currencies_trait::{AssetIdMapping, CurrenciesHandler};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{
		Currency as PalletCurrency, ExistenceRequirement, Get,
		LockableCurrency as PalletLockableCurrency, ReservableCurrency as PalletReservableCurrency,
		WithdrawReasons,
	},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use listen_primitives::CurrencyId;
pub use module::*;
use orml_traits::{
	arithmetic::{Signed, SimpleArithmetic},
	BalanceStatus, BasicCurrency, BasicCurrencyExtended, BasicLockableCurrency,
	BasicReservableCurrency, LockIdentifier, MultiCurrency, MultiCurrencyExtended,
	MultiLockableCurrency, MultiReservableCurrency,
};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	traits::{CheckedSub, MaybeSerializeDeserialize, Saturating, StaticLookup, Zero},
	DispatchError, DispatchResult,
};
use sp_std::{
	boxed::Box,
	convert::{TryFrom, TryInto},
	fmt::Debug,
	marker, result,
	vec::Vec,
};
pub use weights::WeightInfo;
use xcm::{v1::MultiLocation, VersionedMultiLocation};

pub mod currencies_trait;
mod weights;

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, Eq, PartialEq, Default, RuntimeDebug, TypeInfo)]
pub struct ListenAssetMetadata {
	/// project name
	pub name: Vec<u8>,
	/// The ticker symbol for this asset. Limited in length by `StringLimit`.
	pub symbol: Vec<u8>,
	/// The number of decimals this asset uses to represent one unit.
	pub decimals: u8,
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, Eq, PartialEq, Default, RuntimeDebug, TypeInfo)]
pub struct ListenAssetInfo<AccountId, ListenAssetMetadata> {
	pub owner: AccountId,
	pub metadata: Option<ListenAssetMetadata>,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub(crate) type BalanceOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	pub(crate) type AmountOf<T> = <<T as Config>::MultiCurrency as MultiCurrencyExtended<
		<T as frame_system::Config>::AccountId,
	>>::Amount;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type MultiCurrency: MultiCurrency<CurrencyId = CurrencyId, Self::AccountId>
			+ MultiCurrencyExtended<Self::AccountId>
			+ MultiLockableCurrency<Self::AccountId>
			+ MultiReservableCurrency<Self::AccountId>;

		type NativeCurrency: BasicCurrencyExtended<
				Self::AccountId,
				Balance = BalanceOf<Self>,
				Amount = AmountOf<Self>,
			> + BasicLockableCurrency<Self::AccountId, Balance = BalanceOf<Self>>
			+ BasicReservableCurrency<Self::AccountId, Balance = BalanceOf<Self>>;

		type SetLocationOrigin: EnsureOrigin<Self::Origin>;

		type ForceSetLocationOrigin: EnsureOrigin<Self::Origin>;

		#[pallet::constant]
		type GetNativeCurrencyId: Get<CurrencyId>;

		#[pallet::constant]
		type AirDropAmount: Get<BalanceOf<Self>>;

		/// Weight information for extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unable to convert the Amount type into Balance.
		AmountIntoBalanceFailed,
		/// Balance is too low.
		BalanceTooLow,
		/// Asset info is not exists
		AssetAlreadyExists,
		AssetNotExists,
		MetadataNotChange,
		MetadataErr,
		NotOwner,
		ShouldNotChangeDecimals,
		MetadataNotExists,
		NativeCurrency,
		RemainAmountLessThanAirdrop,
		BadLocation,
		MultiLocationExisted,
		AssetIdExisted,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Currency transfer success. [currency_id, from, to, amount]
		Transferred(CurrencyId, T::AccountId, T::AccountId, BalanceOf<T>),
		/// Update balance success. [currency_id, who, amount]
		BalanceUpdated(CurrencyId, T::AccountId, AmountOf<T>),
		/// Deposit success. [currency_id, who, amount]
		Deposited(CurrencyId, T::AccountId, BalanceOf<T>),
		/// Withdraw success. [currency_id, who, amount]
		Withdrawn(CurrencyId, T::AccountId, BalanceOf<T>),
		CreateAsset(T::AccountId, CurrencyId, BalanceOf<T>),
		SetMetadata(T::AccountId, CurrencyId, ListenAssetMetadata),
		Burn(T::AccountId, CurrencyId, BalanceOf<T>),
		SetLocation {
			currency_id: CurrencyId,
			location: MultiLocation,
		},
		ForceSetLocation {
			currency_id: CurrencyId,
			location: MultiLocation,
		},
		SetWeightRateMultiple {
			currency_id: CurrencyId,
			multiple: u128,
		},
	}

	#[pallet::storage]
	/// Metadata of an asset.
	pub(super) type ListenAssetsInfo<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CurrencyId,
		ListenAssetInfo<T::AccountId, ListenAssetMetadata>,
	>;

	#[pallet::storage]
	pub type UsersNumber<T: Config> = StorageMap<_, Identity, CurrencyId, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn asset_locations)]
	pub type AssetLocations<T: Config> =
		StorageMap<_, Twox64Concat, CurrencyId, MultiLocation, OptionQuery>;

	#[pallet::type_value]
	pub fn WeightRateMultipleOnEmpty<T: Config>() -> u128 {
		1000u128
	}

	#[pallet::storage]
	#[pallet::getter(fn weight_rate_multiple)]
	pub type WeightRateMultiple<T: Config> = StorageMap<_, Identity, CurrencyId, u128, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn location_to_currency_ids)]
	pub type LocationToCurrencyIds<T: Config> =
		StorageMap<_, Twox64Concat, MultiLocation, CurrencyId, OptionQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub assets:
			Vec<(CurrencyId, ListenAssetInfo<T::AccountId, ListenAssetMetadata>, BalanceOf<T>)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { assets: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			self.assets.iter().for_each(|asset_info| {
				ListenAssetsInfo::<T>::insert(asset_info.0, asset_info.1.clone());
				T::MultiCurrency::deposit(asset_info.0, &asset_info.1.owner, asset_info.2)
					.expect("can not deposit");
			})
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Users create the asset.
		#[pallet::weight(1500_000_000)]
		pub fn create_asset(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			amount: BalanceOf<T>,
			metadata: Option<ListenAssetMetadata>,
		) -> DispatchResultWithPostInfo {
			let user = ensure_signed(origin)?;

			ensure!(
				!Self::is_exists_metadata(currency_id) &&
					T::MultiCurrency::total_issuance(currency_id) == BalanceOf::<T>::from(0u32),
				Error::<T>::AssetAlreadyExists
			);
			T::MultiCurrency::deposit(currency_id, &user, amount)?;
			ListenAssetsInfo::<T>::insert(
				currency_id,
				ListenAssetInfo { owner: user.clone(), metadata },
			);

			Self::deposit_event(Event::CreateAsset(user.clone(), currency_id, amount));
			Ok(().into())
		}

		/// After setting location, cross-chain transfers can be made
		#[pallet::weight(1500_000_000)]
		pub fn set_location(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			location: Box<VersionedMultiLocation>,
		) -> DispatchResultWithPostInfo {
			T::SetLocationOrigin::ensure_origin(origin)?;
			let location: MultiLocation =
				(*location).try_into().map_err(|()| Error::<T>::BadLocation)?;
			ensure!(ListenAssetsInfo::<T>::contains_key(currency_id), Error::<T>::AssetNotExists);

			ensure!(
				!AssetLocations::<T>::contains_key(currency_id),
				Error::<T>::MultiLocationExisted
			);
			ensure!(
				!LocationToCurrencyIds::<T>::contains_key(location.clone()),
				Error::<T>::AssetIdExisted
			);

			AssetLocations::<T>::insert(currency_id, location.clone());
			LocationToCurrencyIds::<T>::insert(location.clone(), currency_id);
			Self::deposit_event(Event::SetLocation { currency_id, location });
			Ok(().into())
		}

		#[pallet::weight(1500_000_000)]
		pub fn set_weight_rate_multiple(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			multiple: u128,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let info = ListenAssetsInfo::<T>::get(currency_id).ok_or(Error::<T>::AssetNotExists)?;
			ensure!(who == info.owner, Error::<T>::NotOwner);
			WeightRateMultiple::<T>::insert(currency_id, multiple);
			Self::deposit_event(Event::SetWeightRateMultiple { currency_id, multiple });

			Ok(().into())
		}

		/// After setting location, cross-chain transfers can be made
		#[pallet::weight(1500_000_000)]
		pub fn force_set_location(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			location: Box<VersionedMultiLocation>,
		) -> DispatchResultWithPostInfo {
			T::ForceSetLocationOrigin::ensure_origin(origin)?;
			let location: MultiLocation =
				(*location).try_into().map_err(|()| Error::<T>::BadLocation)?;
			ensure!(ListenAssetsInfo::<T>::contains_key(currency_id), Error::<T>::AssetNotExists);
			AssetLocations::<T>::insert(currency_id, location.clone());
			LocationToCurrencyIds::<T>::insert(location.clone(), currency_id);
			Self::deposit_event(Event::ForceSetLocation { currency_id, location });
			Ok(().into())
		}

		/// Users set the asset metadata.
		///
		/// You should have created the asset first.
		#[pallet::weight(1500_000_000)]
		pub fn set_metadata(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			metadata: ListenAssetMetadata,
		) -> DispatchResultWithPostInfo {
			let user = ensure_signed(origin)?;

			ensure!(
				metadata.name.len() > 2 &&
					metadata.symbol.len() > 1 &&
					metadata.decimals > 0u8 &&
					metadata.decimals < 19,
				Error::<T>::MetadataErr
			);

			ListenAssetsInfo::<T>::try_mutate_exists(currency_id, |h| -> DispatchResult {
				let mut info = h.as_mut().take().ok_or(Error::<T>::AssetNotExists)?;
				ensure!(info.owner == user, Error::<T>::NotOwner);
				if let Some(m) = &info.metadata {
					ensure!(m != &metadata, Error::<T>::MetadataNotChange);
					ensure!(m.decimals == metadata.decimals, Error::<T>::ShouldNotChangeDecimals);
				}

				info.metadata = Some(metadata.clone());
				*h = Some(info.clone());
				Self::deposit_event(Event::SetMetadata(user, currency_id, metadata));
				Ok(())
			})?;

			Ok(().into())
		}

		/// Users destroy their own assets.
		#[pallet::weight(1500_000_000)]
		pub fn burn(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let user = ensure_signed(origin)?;

			ensure!(Self::is_exists_metadata(currency_id), Error::<T>::MetadataNotExists);
			T::MultiCurrency::withdraw(currency_id, &user, amount)?;

			Self::deposit_event(Event::Burn(user, currency_id, amount));
			Ok(().into())
		}

		/// Transfer some balance to another account under `currency_id`.
		///
		/// The dispatch origin for this call must be `Signed` by the
		/// transactor.
		#[pallet::weight(T::WeightInfo::transfer_non_native_currency())]
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;

			let to = T::Lookup::lookup(dest)?;
			ensure!(Self::is_exists_metadata(currency_id), Error::<T>::MetadataNotExists);

			<Self as MultiCurrency<T::AccountId>>::transfer(currency_id, &from, &to, amount)?;
			Ok(().into())
		}

		// /// Transfer some native currency to another account.
		// ///
		// /// The dispatch origin for this call must be `Signed` by the
		// /// transactor.
		// #[pallet::weight(T::WeightInfo::transfer_native_currency())]
		// pub fn transfer_native_currency(
		// 	origin: OriginFor<T>,
		// 	dest: <T::Lookup as StaticLookup>::Source,

		// 	#[pallet::compact] amount: BalanceOf<T>,
		// ) -> DispatchResultWithPostInfo {
		// 	let from = ensure_signed(origin)?;
		//
		// 	let to = T::Lookup::lookup(dest)?;
		// 	T::NativeCurrency::transfer(&from, &to, amount)?;
		//
		// 	Self::deposit_event(Event::Transferred(
		// 		T::GetNativeCurrencyId::get(),
		// 		from,
		// 		to,
		// 		amount,
		// 	));
		// 	Ok(().into())
		// }

		/// update amount of account `who` under `currency_id`.
		///
		/// The dispatch origin of this call must be _Root_.
		#[pallet::weight(T::WeightInfo::update_balance_non_native_currency())]
		pub fn update_balance(
			origin: OriginFor<T>,
			who: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyId,
			amount: AmountOf<T>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(Self::is_exists_metadata(currency_id), Error::<T>::MetadataNotExists);
			let dest = T::Lookup::lookup(who)?;
			<Self as MultiCurrencyExtended<T::AccountId>>::update_balance(
				currency_id,
				&dest,
				amount,
			)?;
			Ok(().into())
		}
	}
}

impl<T: Config> CurrenciesHandler<CurrencyId, ListenAssetMetadata, DispatchError> for Pallet<T> {
	fn get_metadata(currency: CurrencyId) -> Result<ListenAssetMetadata, DispatchError> {
		Ok(Self::get_metadata(currency)?)
	}
}

impl<T: Config> Pallet<T> {
	fn is_exists_metadata(currency_id: CurrencyId) -> bool {
		if let Some(x) = ListenAssetsInfo::<T>::get(currency_id) {
			if let Some(_) = x.metadata {
				return true
			}
		}
		false
	}

	fn get_metadata(currency_id: CurrencyId) -> result::Result<ListenAssetMetadata, DispatchError> {
		match ListenAssetsInfo::<T>::get(currency_id)
			.ok_or(Error::<T>::AssetNotExists)?
			.metadata
		{
			Some(a) => Ok(a),
			_ => Err(Error::<T>::MetadataNotExists)?,
		}
	}
}

impl<T: Config> MultiCurrency<T::AccountId> for Pallet<T> {
	type CurrencyId = CurrencyId;
	type Balance = BalanceOf<T>;

	fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::minimum_balance()
		} else {
			T::MultiCurrency::minimum_balance(currency_id)
		}
	}

	fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::total_issuance()
		} else {
			T::MultiCurrency::total_issuance(currency_id)
		}
	}

	fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::total_balance(who)
		} else {
			T::MultiCurrency::total_balance(currency_id, who)
		}
	}

	fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::free_balance(who)
		} else {
			T::MultiCurrency::free_balance(currency_id, who)
		}
	}

	fn ensure_can_withdraw(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::ensure_can_withdraw(who, amount)
		} else {
			T::MultiCurrency::ensure_can_withdraw(currency_id, who, amount)
		}
	}

	fn transfer(
		currency_id: Self::CurrencyId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if amount.is_zero() || from == to {
			return Ok(())
		}
		if currency_id == T::GetNativeCurrencyId::get() {
			ensure!(
				Self::total_balance(T::GetNativeCurrencyId::get(), &from).saturating_sub(amount) >
					T::AirDropAmount::get(),
				Error::<T>::RemainAmountLessThanAirdrop
			);
			T::NativeCurrency::transfer(from, to, amount)?;
		} else {
			T::MultiCurrency::transfer(currency_id, from, to, amount)?;
		}
		Self::deposit_event(Event::Transferred(currency_id, from.clone(), to.clone(), amount));
		Ok(())
	}

	fn deposit(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if amount.is_zero() {
			return Ok(())
		}
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::deposit(who, amount)?;
		} else {
			T::MultiCurrency::deposit(currency_id, who, amount)?;
		}
		Self::deposit_event(Event::Deposited(currency_id, who.clone(), amount));
		Ok(())
	}

	fn withdraw(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if amount.is_zero() {
			return Ok(())
		}
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::withdraw(who, amount)?;
		} else {
			T::MultiCurrency::withdraw(currency_id, who, amount)?;
		}
		Self::deposit_event(Event::Withdrawn(currency_id, who.clone(), amount));
		Ok(())
	}

	fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::can_slash(who, amount)
		} else {
			T::MultiCurrency::can_slash(currency_id, who, amount)
		}
	}

	fn slash(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::slash(who, amount)
		} else {
			T::MultiCurrency::slash(currency_id, who, amount)
		}
	}
}

impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
	type Amount = AmountOf<T>;

	fn update_balance(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		by_amount: Self::Amount,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::update_balance(who, by_amount)?;
		} else {
			T::MultiCurrency::update_balance(currency_id, who, by_amount)?;
		}
		Self::deposit_event(Event::BalanceUpdated(currency_id, who.clone(), by_amount));
		Ok(())
	}
}

impl<T: Config> MultiLockableCurrency<T::AccountId> for Pallet<T> {
	type Moment = T::BlockNumber;

	fn set_lock(
		lock_id: LockIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::set_lock(lock_id, who, amount)
		} else {
			T::MultiCurrency::set_lock(lock_id, currency_id, who, amount)
		}
	}

	fn extend_lock(
		lock_id: LockIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::extend_lock(lock_id, who, amount)
		} else {
			T::MultiCurrency::extend_lock(lock_id, currency_id, who, amount)
		}
	}

	fn remove_lock(
		lock_id: LockIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::remove_lock(lock_id, who)
		} else {
			T::MultiCurrency::remove_lock(lock_id, currency_id, who)
		}
	}
}

impl<T: Config> MultiReservableCurrency<T::AccountId> for Pallet<T> {
	fn can_reserve(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		value: Self::Balance,
	) -> bool {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::can_reserve(who, value)
		} else {
			T::MultiCurrency::can_reserve(currency_id, who, value)
		}
	}

	fn slash_reserved(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		value: Self::Balance,
	) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::slash_reserved(who, value)
		} else {
			T::MultiCurrency::slash_reserved(currency_id, who, value)
		}
	}

	fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::reserved_balance(who)
		} else {
			T::MultiCurrency::reserved_balance(currency_id, who)
		}
	}

	fn reserve(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		value: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::reserve(who, value)
		} else {
			T::MultiCurrency::reserve(currency_id, who, value)
		}
	}

	fn unreserve(
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		value: Self::Balance,
	) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::unreserve(who, value)
		} else {
			T::MultiCurrency::unreserve(currency_id, who, value)
		}
	}

	fn repatriate_reserved(
		currency_id: Self::CurrencyId,
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::repatriate_reserved(slashed, beneficiary, value, status)
		} else {
			T::MultiCurrency::repatriate_reserved(currency_id, slashed, beneficiary, value, status)
		}
	}
}

pub struct Currency<T, GetCurrencyId>(marker::PhantomData<T>, marker::PhantomData<GetCurrencyId>);

impl<T, GetCurrencyId> BasicCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyId>,
{
	type Balance = BalanceOf<T>;

	fn minimum_balance() -> Self::Balance {
		<Pallet<T>>::minimum_balance(GetCurrencyId::get())
	}

	fn total_issuance() -> Self::Balance {
		<Pallet<T>>::total_issuance(GetCurrencyId::get())
	}

	fn total_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T>>::total_balance(GetCurrencyId::get(), who)
	}

	fn free_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T>>::free_balance(GetCurrencyId::get(), who)
	}

	fn ensure_can_withdraw(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::ensure_can_withdraw(GetCurrencyId::get(), who, amount)
	}

	fn transfer(from: &T::AccountId, to: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T> as MultiCurrency<T::AccountId>>::transfer(GetCurrencyId::get(), from, to, amount)
	}

	fn deposit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::deposit(GetCurrencyId::get(), who, amount)
	}

	fn withdraw(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::withdraw(GetCurrencyId::get(), who, amount)
	}

	fn can_slash(who: &T::AccountId, amount: Self::Balance) -> bool {
		<Pallet<T>>::can_slash(GetCurrencyId::get(), who, amount)
	}

	fn slash(who: &T::AccountId, amount: Self::Balance) -> Self::Balance {
		<Pallet<T>>::slash(GetCurrencyId::get(), who, amount)
	}
}

impl<T, GetCurrencyId> BasicCurrencyExtended<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyId>,
{
	type Amount = AmountOf<T>;

	fn update_balance(who: &T::AccountId, by_amount: Self::Amount) -> DispatchResult {
		<Pallet<T> as MultiCurrencyExtended<T::AccountId>>::update_balance(
			GetCurrencyId::get(),
			who,
			by_amount,
		)
	}
}

impl<T, GetCurrencyId> BasicLockableCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyId>,
{
	type Moment = T::BlockNumber;

	fn set_lock(
		lock_id: LockIdentifier,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		<Pallet<T> as MultiLockableCurrency<T::AccountId>>::set_lock(
			lock_id,
			GetCurrencyId::get(),
			who,
			amount,
		)
	}

	fn extend_lock(
		lock_id: LockIdentifier,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		<Pallet<T> as MultiLockableCurrency<T::AccountId>>::extend_lock(
			lock_id,
			GetCurrencyId::get(),
			who,
			amount,
		)
	}

	fn remove_lock(lock_id: LockIdentifier, who: &T::AccountId) -> DispatchResult {
		<Pallet<T> as MultiLockableCurrency<T::AccountId>>::remove_lock(
			lock_id,
			GetCurrencyId::get(),
			who,
		)
	}
}

impl<T, GetCurrencyId> BasicReservableCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyId>,
{
	fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::can_reserve(
			GetCurrencyId::get(),
			who,
			value,
		)
	}

	fn slash_reserved(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::slash_reserved(
			GetCurrencyId::get(),
			who,
			value,
		)
	}

	fn reserved_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::reserved_balance(
			GetCurrencyId::get(),
			who,
		)
	}

	fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::reserve(
			GetCurrencyId::get(),
			who,
			value,
		)
	}

	fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::unreserve(
			GetCurrencyId::get(),
			who,
			value,
		)
	}

	fn repatriate_reserved(
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::repatriate_reserved(
			GetCurrencyId::get(),
			slashed,
			beneficiary,
			value,
			status,
		)
	}
}

pub type NativeCurrencyOf<T> = Currency<T, <T as Config>::GetNativeCurrencyId>;

/// Adapt other currency traits implementation to `BasicCurrency`.
pub struct BasicCurrencyAdapter<T, Currency, Amount, Moment>(
	marker::PhantomData<(T, Currency, Amount, Moment)>,
);

type PalletBalanceOf<A, Currency> = <Currency as PalletCurrency<A>>::Balance;

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> BasicCurrency<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: PalletCurrency<AccountId>,
	T: Config,
{
	type Balance = PalletBalanceOf<AccountId, Currency>;

	fn minimum_balance() -> Self::Balance {
		Currency::minimum_balance()
	}

	fn total_issuance() -> Self::Balance {
		Currency::total_issuance()
	}

	fn total_balance(who: &AccountId) -> Self::Balance {
		Currency::total_balance(who)
	}

	fn free_balance(who: &AccountId) -> Self::Balance {
		Currency::free_balance(who)
	}

	fn ensure_can_withdraw(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		let new_balance =
			Self::free_balance(who).checked_sub(&amount).ok_or(Error::<T>::BalanceTooLow)?;

		Currency::ensure_can_withdraw(who, amount, WithdrawReasons::all(), new_balance)
	}

	fn transfer(from: &AccountId, to: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::transfer(from, to, amount, ExistenceRequirement::AllowDeath)
	}

	fn deposit(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		let _ = Currency::deposit_creating(who, amount);
		Ok(())
	}

	fn withdraw(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::withdraw(who, amount, WithdrawReasons::all(), ExistenceRequirement::AllowDeath)
			.map(|_| ())
	}

	fn can_slash(who: &AccountId, amount: Self::Balance) -> bool {
		Currency::can_slash(who, amount)
	}

	fn slash(who: &AccountId, amount: Self::Balance) -> Self::Balance {
		let (_, gap) = Currency::slash(who, amount);
		gap
	}
}

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> BasicCurrencyExtended<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Amount: Signed
		+ TryInto<PalletBalanceOf<AccountId, Currency>>
		+ TryFrom<PalletBalanceOf<AccountId, Currency>>
		+ SimpleArithmetic
		+ Codec
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ MaxEncodedLen
		+ Default,

	Currency: PalletCurrency<AccountId>,
	T: Config,
{
	type Amount = Amount;

	fn update_balance(who: &AccountId, by_amount: Self::Amount) -> DispatchResult {
		let by_balance =
			by_amount.abs().try_into().map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;
		if by_amount.is_positive() {
			Self::deposit(who, by_balance)
		} else {
			Self::withdraw(who, by_balance)
		}
	}
}

// Adapt `frame_support::traits::LockableCurrency`
impl<T, AccountId, Currency, Amount, Moment> BasicLockableCurrency<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: PalletLockableCurrency<AccountId>,
	T: Config,
{
	type Moment = Moment;

	fn set_lock(lock_id: LockIdentifier, who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::set_lock(lock_id, who, amount, WithdrawReasons::all());
		Ok(())
	}

	fn extend_lock(
		lock_id: LockIdentifier,
		who: &AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		Currency::extend_lock(lock_id, who, amount, WithdrawReasons::all());
		Ok(())
	}

	fn remove_lock(lock_id: LockIdentifier, who: &AccountId) -> DispatchResult {
		Currency::remove_lock(lock_id, who);
		Ok(())
	}
}

// Adapt `frame_support::traits::ReservableCurrency`
impl<T, AccountId, Currency, Amount, Moment> BasicReservableCurrency<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: PalletReservableCurrency<AccountId>,
	T: Config,
{
	fn can_reserve(who: &AccountId, value: Self::Balance) -> bool {
		Currency::can_reserve(who, value)
	}

	fn slash_reserved(who: &AccountId, value: Self::Balance) -> Self::Balance {
		let (_, gap) = Currency::slash_reserved(who, value);
		gap
	}

	fn reserved_balance(who: &AccountId) -> Self::Balance {
		Currency::reserved_balance(who)
	}

	fn reserve(who: &AccountId, value: Self::Balance) -> DispatchResult {
		Currency::reserve(who, value)
	}

	fn unreserve(who: &AccountId, value: Self::Balance) -> Self::Balance {
		Currency::unreserve(who, value)
	}

	fn repatriate_reserved(
		slashed: &AccountId,
		beneficiary: &AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		Currency::repatriate_reserved(slashed, beneficiary, value, status)
	}
}

pub struct AssetIdMaps<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> AssetIdMapping<CurrencyId, MultiLocation> for AssetIdMaps<T> {
	fn get_multi_location(asset_id: CurrencyId) -> Option<MultiLocation> {
		Pallet::<T>::asset_locations(asset_id)
	}

	fn get_currency_id(multi_location: MultiLocation) -> Option<CurrencyId> {
		Pallet::<T>::location_to_currency_ids(multi_location)
	}

	fn get_weight_rate_multiple(location: MultiLocation) -> Option<u128> {
		if let Some(id) = Self::get_currency_id(location.clone()) {
			Some(WeightRateMultiple::<T>::get(id))
		} else {
			None
		}
	}
}
