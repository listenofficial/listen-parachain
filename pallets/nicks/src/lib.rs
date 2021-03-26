
#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::prelude::*;
use sp_runtime::{
	traits::{StaticLookup, Zero}
};
use frame_support::{
	decl_module, decl_event, decl_storage, ensure, decl_error,
	traits::{Currency, ReservableCurrency, OnUnbalanced, Get, EnsureOrigin},
	weights::Weight,
};
use frame_system::{self as system, ensure_signed, ensure_root};

type BalanceOf<T> = <<T as Config>::NicksCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::NicksCurrency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

pub trait Config: system::Config {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	/// The currency trait.
	type NicksCurrency: ReservableCurrency<Self::AccountId>;

	/// Reservation fee.
	type ReservationFee: Get<BalanceOf<Self>>;  // 1 token

	/// What to do with slashed funds.
	type Slashed: OnUnbalanced<NegativeImbalanceOf<Self>>;

	/// The origin which may forcibly set or remove a name. Root can always do this.
	type ForceOrigin: EnsureOrigin<Self::Origin>;

	/// The minimum length a name may be.
	type MinLength: Get<usize>;

	/// The maximum length a name may be.
	type MaxLength: Get<usize>;
}

decl_storage! {
	trait Store for Module<T: Config> as Sudo {

		/// 每个account_id对应的名字
		NameOf: map hasher(blake2_128_concat) T::AccountId => Option<(Vec<u8>, BalanceOf<T>)>;

		/// 每个名字对应的account_id
		pub AccountIdOf: map hasher(blake2_128_concat) Vec<u8> => T::AccountId;
	}
}

decl_error! {
	/// Error for the elections module.
	pub enum Error for Module<T: Config> {
		/// 名字太短
		NameTooShort,

		/// 名字太长
		NameTooLong,

		/// 已经存在的名字
		ExistsName,

		/// 不是已经存在的名字
		NotExistsName,

		/// 不是允许的源
		BabOrigin,

		///已经设置名字
		AlreadySetName,

	}
	}


decl_event!(
	pub enum Event<T> where AccountId = <T as system::Config>::AccountId, Balance = BalanceOf<T> {
		/// A name was set.
		NameSet(AccountId),
		/// A name was forcibly set.
		NameForced(AccountId),
		/// A name was changed.
		NameChanged(AccountId),
		/// A name was cleared, and the given balance returned.
		NameCleared(AccountId, Balance),
		/// A name was removed and the given balance slashed.
		NameKilled(AccountId, Balance),
	}
);



decl_module! {
	// Simple declaration of the `Module` type. Lets the macro know what it's working on.
	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		type Error = Error<T>;
		fn deposit_event() = default;

		/// Reservation fee.
		const ReservationFee: BalanceOf<T> = T::ReservationFee::get();

		/// The minimum length a name may be.
		const MinLength: u32 = T::MinLength::get() as u32;

		/// The maximum length a name may be.
		const MaxLength: u32 = T::MaxLength::get() as u32;

		#[weight = 50_000]
		fn set_name(origin, name: Vec<u8>) {
			let sender = ensure_signed(origin)?;

			ensure!(name.len() >= T::MinLength::get(), Error::<T>::NameTooShort);
			ensure!(name.len() <= T::MaxLength::get(), Error::<T>::NameTooLong);

			// 名字不能用相同
			ensure!(!<AccountIdOf<T>>::contains_key(name.clone()),Error::<T>::ExistsName);
			ensure!(!<NameOf<T>>::contains_key(&sender), Error::<T>::AlreadySetName);

			let deposit = T::ReservationFee::get();
			T::NicksCurrency::reserve(&sender, deposit.clone())?;
			Self::deposit_event(RawEvent::NameChanged(sender.clone()));

			<NameOf<T>>::insert(&sender, (name.clone(), deposit));
			<AccountIdOf<T>>::insert(name.clone(), sender.clone());
		}


		#[weight = 70_000]
		fn kill_name(origin, target: <T::Lookup as StaticLookup>::Source) {

			T::ForceOrigin::ensure_origin(origin)?;

			// Figure out who we're meant to be clearing.
			let target = T::Lookup::lookup(target)?;
			// Grab their deposit (and check that they have one).
			let account_info = <NameOf<T>>::take(&target).ok_or(Error::<T>::NotExistsName)?;
			let deposit = account_info.1;
			let name = account_info.0;
			<AccountIdOf<T>>::remove(name);

			// Slash their deposit from them.
			T::Slashed::on_unbalanced(T::NicksCurrency::slash_reserved(&target, deposit.clone()).0);

			Self::deposit_event(RawEvent::NameKilled(target, deposit));
		}


		#[weight = 70_000]
		fn force_name(origin, target: <T::Lookup as StaticLookup>::Source, name: Vec<u8>) {

			T::ForceOrigin::ensure_origin(origin)?;

			let target = T::Lookup::lookup(target)?;

			let deposit = <NameOf<T>>::get(&target).map(|x| x.1).unwrap_or_else(Zero::zero);
			<NameOf<T>>::insert(&target, (name.clone(), deposit));

			// 如果这个名字已经被占用
			if let old_id = <AccountIdOf<T>>::get(name.clone()) {
					// 如果不是他本人占用
					if old_id.clone() != target.clone(){
						if let Some(account_info) = <NameOf<T>>::get(&old_id){

						let old_name = account_info.clone().0;
						let old_deposit = account_info.clone().1;

						<AccountIdOf<T>>::remove(old_name.clone());

						// 归还old_name抵押
						let _ = T::NicksCurrency::unreserve(&old_id, old_deposit.clone());

						 <NameOf<T>>::remove(&old_id);

			}
					}

			}

			<AccountIdOf<T>>::insert(name.clone(), target.clone());

			Self::deposit_event(RawEvent::NameForced(target));
		}
	}
}