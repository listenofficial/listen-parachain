
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_error, decl_event, weights::{Weight}, traits::{Get, Currency, ExistenceRequirement}, ensure};
use frame_system::{self as system, ensure_signed};
use orml_tokens;
use orml_traits::{MultiCurrency};
pub use pallet_balances;

use sp_std::{result::Result, convert::{Into, TryInto, TryFrom}};
use codec::{Encode, Decode};
use sp_runtime::{traits::StaticLookup, RuntimeDebug, SaturatedConversion};
use node_primitives::CurrencyId;


pub(crate) type CurrencyIdOf<T> =
		<<T as Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;

pub(crate) type BalanceOf<T> =
		<<T as Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

type NativeBalanceOf<T> = <<T as Config>::NativeCurrency as Currency<<T as system::Config>::AccountId>>::Balance;


#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum Tokens {
	LT,
	KSM,
	DOT,
	BTC,
	Other(CurrencyId),
}

impl Default for Tokens {
	fn default() -> Self {
		Self::LT
	}
}

impl Into<CurrencyId> for Tokens {
	fn into(self) -> u32 {
		match self {
			Tokens::LT => 0 as CurrencyId,
			Tokens::BTC =>1 as CurrencyId,
			Tokens::KSM => 2 as CurrencyId,
			Tokens::DOT => 3 as CurrencyId,
			Tokens::Other( x) => x as CurrencyId,
		}
	}
}



pub trait Config: system::Config {

	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	type NativeCurrency: Currency<Self::AccountId>;

	type GetNativeCurrencyId: Get<CurrencyIdOf<Self>>;

	type MultiCurrency: MultiCurrency<Self::AccountId>;

	type AirDropAmount: Get<NativeBalanceOf<Self>>;

    // type Test: CurrencyIdOf<T>;

}

decl_event!(
	pub enum Event<T> where
	 <T as system::Config>::AccountId,
	  CurrencyId = CurrencyIdOf<T>,{
	 	Transfer(CurrencyId, AccountId, AccountId, u128),
	 });


decl_module! {

	pub struct Module<T: Config> for enum Call where origin: T::Origin {

		/// 本链资产id
		const GetNativeCurrencyId: CurrencyIdOf<T> = T::GetNativeCurrencyId::get();

		/// 账户上可以转账的最小剩余资产
		const AirDropAmount: NativeBalanceOf<T> = T::AirDropAmount::get();

		type Error = Error<T>;
		fn deposit_event() = default;

		#[weight = 10_000]
		pub fn transfer(
			origin,
			dest: <T::Lookup as StaticLookup>::Source,
			token: Tokens,
			amount: BalanceOf<T>,
		) {

			let currency_id: u32 = token.into();

			let currency_id = <CurrencyIdOf<T>>::from(currency_id);

			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;
			let amount_u128 = amount.saturated_into::<u128>();
			if currency_id != T::GetNativeCurrencyId::get() {
				T::MultiCurrency::transfer(currency_id, &from, &to, amount)?;
			}
			else {

				// 转金额类型
				let amount = amount_u128.saturated_into::<NativeBalanceOf<T>>();
				
				// 获取账上自由金额
				
				
				// 判断金额是否大于0.99个
				if T::AirDropAmount::get() < amount {
					T::NativeCurrency::transfer(&from, &to, amount, ExistenceRequirement::AllowDeath)?;
				}
				else {
					return Err(Error::<T>::AmountTooLow)?;

				}


			}

			Self::deposit_event(RawEvent::Transfer(currency_id, from, to, amount_u128));

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
		ConvertErr,


	 }

}
//
