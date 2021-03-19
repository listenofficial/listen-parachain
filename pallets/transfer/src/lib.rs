
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_error, decl_event, weights::{Weight}, traits::{Get, Currency, ExistenceRequirement, ReservableCurrency}, ensure};
use frame_system::{self as system, ensure_signed, ensure_root};
use orml_tokens;
use orml_traits::{MultiCurrency, MultiCurrencyExtended, BasicCurrency};
pub use pallet_balances;
use sp_runtime::traits::Saturating;

use sp_std::{result::Result, convert::{Into, TryInto, TryFrom}};
use codec::{Encode, Decode};
use sp_runtime::{traits::StaticLookup, RuntimeDebug, SaturatedConversion};
use node_primitives::*;
use orml_currencies;

pub(crate) type AmountOf<T> =
		<<T as orml_currencies::Config>::MultiCurrency as MultiCurrencyExtended<<T as frame_system::Config>::AccountId>>::Amount;

pub(crate) type CurrencyIdOf<T> =
		<<T as orml_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;

pub(crate) type BalanceOf<T> =
		<<T as orml_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

pub trait Config: system::Config + orml_currencies::Config {

	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	type AirDropAmount: Get<BalanceOf<Self>>;

}

decl_event!(
	pub enum Event<T> where
	 	AccountId = <T as system::Config>::AccountId,
		Amount = BalanceOf<T>,
	  	CurrencyId = CurrencyIdOf<T>,{
	 	Transferred(CurrencyId, AccountId, AccountId, Amount),
	 });


decl_module! {

	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		/// 本链资产id
		const GetNativeCurrencyId: CurrencyIdOf<T> = T::GetNativeCurrencyId::get();

		// /// 账户上可以转账的最小剩余资产
		const AirDropAmount: BalanceOf<T> = T::AirDropAmount::get();

		type Error = Error<T>;
		fn deposit_event() = default;

		#[weight = 1000]
		pub fn transfer(
			origin,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyIdOf<T>,
			amount: BalanceOf<T>,
		)  {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			if T::GetNativeCurrencyId::get() == currency_id {

				ensure!(T::MultiCurrency::total_balance(currency_id, &from) > T::AirDropAmount::get(), Error::<T>::AmountTooLow);
			}

			T::MultiCurrency::transfer(currency_id, &from, &to, amount)?;

		}


		#[weight = 1000]
		pub fn transfer_native_currency(
			origin,
			dest: <T::Lookup as StaticLookup>::Source,
			amount: BalanceOf<T>,
		) {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			ensure!(T::MultiCurrency::total_balance(T::GetNativeCurrencyId::get(), &from) > T::AirDropAmount::get(), Error::<T>::AmountTooLow);
			T::NativeCurrency::transfer(&from, &to, amount)?;

			Self::deposit_event(RawEvent::Transferred(T::GetNativeCurrencyId::get(), from, to, amount));
		}

		#[weight = 1000]
		pub fn update_balance(
			origin,
			who: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyIdOf<T>,
			amount: AmountOf<T>,
		) {
			ensure_root(origin)?;
			let dest = T::Lookup::lookup(who)?;
			T::MultiCurrency::update_balance(currency_id, &dest, amount)?;

		}

		}
		}

decl_error!{
	pub enum Error for Module<T: Config> {
		/// 是本链的资产
		NativeCurrency,
		/// 代币错误(不存在)
		TokenNotExist,
		/// 金额太小
		AmountTooLow,
		///
		TokenErr,
	 }

}
//
