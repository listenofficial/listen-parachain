// Forked from https://github.com/paritytech/substrate/blob/master/frame/nicks/src/lib.rs

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

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{Currency, EnsureOrigin, ExistenceRequirement, Get, OnUnbalanced, ReservableCurrency},
	weights::Weight,
	PalletId,
};
use frame_system::{self as system, ensure_root, ensure_signed};
use scale_info::TypeInfo;
use sp_runtime::traits::{AccountIdConversion, StaticLookup, Zero};
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	type BalanceOf<T> =
		<<T as Config>::NicksCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	type NegativeImbalanceOf<T> = <<T as Config>::NicksCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	#[pallet::config]
	#[pallet::disable_frame_system_supertrait_check]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>>
			+ Into<<Self as system::Config>::Event>
			+ IsType<<Self as frame_system::Config>::Event>;

		/// The currency trait.
		type NicksCurrency: ReservableCurrency<Self::AccountId>;

		/// What to do with slashed funds.
		type Slashed: OnUnbalanced<NegativeImbalanceOf<Self>>;

		/// The origin which may forcibly set or remove a name. Root can always do this.
		type ForceOrigin: EnsureOrigin<Self::Origin>;

		#[pallet::constant]
		type NameFee: Get<BalanceOf<Self>>;

		#[pallet::constant]
		type MinLength: Get<u32>;

		#[pallet::constant]
		type MaxLength: Get<u32>;

		#[pallet::constant]
		type TreasuryPalletId: Get<PalletId>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(1500_000_000)]
		pub fn set_name(origin: OriginFor<T>, name: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(name.len() as u32 >= T::MinLength::get(), Error::<T>::NameTooShort);
			ensure!(name.len() as u32 <= T::MaxLength::get(), Error::<T>::NameTooLong);

			ensure!(!<AccountIdOf<T>>::contains_key(name.clone()), Error::<T>::ExistsName);
			ensure!(!<NameOf<T>>::contains_key(&sender), Error::<T>::AlreadySetName);

			let Fee = T::NameFee::get();
			T::NicksCurrency::transfer(
				&sender,
				&Self::treasury_account_id(),
				Fee.clone(),
				ExistenceRequirement::KeepAlive,
			)?;
			Self::deposit_event(Event::NameChanged(sender.clone()));

			<NameOf<T>>::insert(&sender, (name.clone(), Fee));
			<AccountIdOf<T>>::insert(name.clone(), sender.clone());
			Ok(())
		}

		#[pallet::weight(1500_000_000)]
		pub fn kill_name(
			origin: OriginFor<T>,
			target: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;

			let target = T::Lookup::lookup(target)?;
			let account_info = <NameOf<T>>::take(&target).ok_or(Error::<T>::NotExistsName)?;
			<AccountIdOf<T>>::remove(account_info.0);

			Self::deposit_event(Event::NameKilled(target));
			Ok(())
		}

		#[pallet::weight(1500_000_000)]
		pub fn force_name(
			origin: OriginFor<T>,
			target: <T::Lookup as StaticLookup>::Source,
			name: Vec<u8>,
		) -> DispatchResult {
			T::ForceOrigin::ensure_origin(origin)?;

			let target = T::Lookup::lookup(target)?;
			let deposit = <NameOf<T>>::get(&target).map(|x| x.1).unwrap_or_else(Zero::zero);
			<NameOf<T>>::insert(&target, (name.clone(), deposit));

			if let old_id = <AccountIdOf<T>>::get(name.clone()) {
				if old_id.clone() != target.clone() {
					if let Some(account_info) = <NameOf<T>>::get(&old_id) {
						let old_name = account_info.clone().0;
						let old_fee = account_info.clone().1;
						<AccountIdOf<T>>::remove(old_name.clone());

						let _ = T::NicksCurrency::transfer(
							&Self::treasury_account_id(),
							&old_id,
							old_fee.clone(),
							ExistenceRequirement::KeepAlive,
						);
						<NameOf<T>>::remove(&old_id);
					}
				}
			}
			<AccountIdOf<T>>::insert(name.clone(), target.clone());

			Self::deposit_event(Event::NameForced(target));
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn treasury_account_id() -> T::AccountId {
			T::TreasuryPalletId::get().into_account()
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		NameTooShort,
		NameTooLong,
		ExistsName,
		NotExistsName,
		BabOrigin,
		AlreadySetName,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A name was set.
		NameSet(T::AccountId),
		/// A name was forcibly set.
		NameForced(T::AccountId),
		/// A name was changed.
		NameChanged(T::AccountId),
		/// A name was cleared, and the given balance returned.
		NameCleared(T::AccountId, BalanceOf<T>),
		/// A name was removed and the given balance slashed.
		NameKilled(T::AccountId),
	}

	#[pallet::storage]
	#[pallet::getter(fn name_of)]
	pub type NameOf<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, (Vec<u8>, BalanceOf<T>), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn account_id_of)]
	pub type AccountIdOf<T: Config> =
		StorageMap<_, Blake2_128Concat, Vec<u8>, T::AccountId, ValueQuery>;
}
