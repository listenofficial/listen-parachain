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

type BalanceOf<T> =
	<<T as Config>::NicksCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::NicksCurrency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub trait Config: system::Config {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	/// The currency trait.
	type NicksCurrency: ReservableCurrency<Self::AccountId>;

	type NameFee: Get<BalanceOf<Self>>; // 1 token

	/// What to do with slashed funds.
	type Slashed: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// The origin which may forcibly set or remove a name. Root can always do this.
	type ForceOrigin: EnsureOrigin<Self::Origin>;

	/// The minimum length a name may be.
	type MinLength: Get<usize>;

	/// The maximum length a name may be.
	type MaxLength: Get<usize>;

	type TreasuryPalletId: Get<PalletId>;
}

decl_storage! {
	trait Store for Module<T: Config> as Sudo {

		pub NameOf: map hasher(blake2_128_concat) T::AccountId => Option<(Vec<u8>, BalanceOf<T>)>;
		pub AccountIdOf: map hasher(blake2_128_concat) Vec<u8> => T::AccountId;
	}
}

decl_error! {
/// Error for the elections module.
pub enum Error for Module<T: Config> {
	NameTooShort,
	NameTooLong,
	ExistsName,
	NotExistsName,
	BabOrigin,
	AlreadySetName,

}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as system::Config>::AccountId,
		Balance = BalanceOf<T>,
	{
		/// A name was set.
		NameSet(AccountId),
		/// A name was forcibly set.
		NameForced(AccountId),
		/// A name was changed.
		NameChanged(AccountId),
		/// A name was cleared, and the given balance returned.
		NameCleared(AccountId, Balance),
		/// A name was removed and the given balance slashed.
		NameKilled(AccountId),
	}
);

decl_module! {
	// Simple declaration of the `Module` type. Lets the macro know what it's working on.
	#[derive(TypeInfo)]
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		type Error = Error<T>;
		fn deposit_event() = default;

		const NameFee: BalanceOf<T> = T::NameFee::get();

		/// The minimum length a name may be.
		const MinLength: u32 = T::MinLength::get() as u32;

		/// The maximum length a name may be.
		const MaxLength: u32 = T::MaxLength::get() as u32;

		/// User set their nicks name.
		///
		/// Notice: the name cannot be changed after being set.
		#[weight = 50_000]
		fn set_name(origin, name: Vec<u8>) {
			let sender = ensure_signed(origin)?;

			ensure!(name.len() >= T::MinLength::get(), Error::<T>::NameTooShort);
			ensure!(name.len() <= T::MaxLength::get(), Error::<T>::NameTooLong);

			ensure!(!<AccountIdOf<T>>::contains_key(name.clone()),Error::<T>::ExistsName);
			ensure!(!<NameOf<T>>::contains_key(&sender), Error::<T>::AlreadySetName);

			let Fee = T::NameFee::get();
			T::NicksCurrency::transfer(&sender, &Self::treasury_account_id(), Fee.clone(), ExistenceRequirement::KeepAlive)?;
			Self::deposit_event(RawEvent::NameChanged(sender.clone()));

			<NameOf<T>>::insert(&sender, (name.clone(), Fee));
			<AccountIdOf<T>>::insert(name.clone(), sender.clone());
		}


		/// The council members kill the nicks name of the user.
		///
		/// It will return your fee.
		#[weight = 70_000]
		fn kill_name(origin, target: <T::Lookup as StaticLookup>::Source) {
			T::ForceOrigin::ensure_origin(origin)?;

			let target = T::Lookup::lookup(target)?;
			let account_info = <NameOf<T>>::take(&target).ok_or(Error::<T>::NotExistsName)?;
			<AccountIdOf<T>>::remove(account_info.0);

			Self::deposit_event(RawEvent::NameKilled(target));
		}


		/// The council members force set a nicks name for the user.
		///
		/// It overrides your previous Settings
		#[weight = 70_000]
		fn force_name(origin, target: <T::Lookup as StaticLookup>::Source, name: Vec<u8>) {
			T::ForceOrigin::ensure_origin(origin)?;

			let target = T::Lookup::lookup(target)?;
			let deposit = <NameOf<T>>::get(&target).map(|x| x.1).unwrap_or_else(Zero::zero);
			<NameOf<T>>::insert(&target, (name.clone(), deposit));

			if let old_id = <AccountIdOf<T>>::get(name.clone()) {
					if old_id.clone() != target.clone(){
						if let Some(account_info) = <NameOf<T>>::get(&old_id){
						let old_name = account_info.clone().0;
						let old_fee = account_info.clone().1;
						<AccountIdOf<T>>::remove(old_name.clone());

						let _ = T::NicksCurrency::transfer(&Self::treasury_account_id(), &old_id, old_fee.clone(), ExistenceRequirement::KeepAlive);
						 <NameOf<T>>::remove(&old_id);
			}
					}

			}
			<AccountIdOf<T>>::insert(name.clone(), target.clone());

			Self::deposit_event(RawEvent::NameForced(target));
		}
	}
}

impl<T: Config> Module<T> {
	pub fn treasury_account_id() -> T::AccountId {
		T::TreasuryPalletId::get().into_account()
	}
}
