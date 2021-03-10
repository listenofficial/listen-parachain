
#![cfg_attr(not(feature = "std"), no_std)]

pub mod vesting_traits;
use vesting_traits::VestingSchedule;
use sp_std::prelude::*;
use sp_std::fmt::Debug;

use codec::{Encode, Decode};
use sp_runtime::{DispatchResult, RuntimeDebug, traits::{
	StaticLookup, Zero, AtLeast32BitUnsigned, MaybeSerializeDeserialize, Convert
}};
use frame_support::{decl_module, decl_event, decl_storage, decl_error, ensure, weights::Weight};
use frame_support::traits::{
	Currency, LockableCurrency, WithdrawReasons, LockIdentifier,
	ExistenceRequirement, Get,
};

use frame_system::{ensure_signed, ensure_root};

mod benchmarking;
mod default_weights;

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type MaxLocksOf<T> = <<T as Config>::Currency as LockableCurrency<<T as frame_system::Config>::AccountId>>::MaxLocks;

pub trait WeightInfo {
	fn vest_locked(l: u32, ) -> Weight;
	fn vest_unlocked(l: u32, ) -> Weight;
	fn vest_other_locked(l: u32, ) -> Weight;
	fn vest_other_unlocked(l: u32, ) -> Weight;
	fn vested_transfer(l: u32, ) -> Weight;
	fn force_vested_transfer(l: u32, ) -> Weight;
}

pub trait Config: frame_system::Config {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// The currency trait.
	type Currency: LockableCurrency<Self::AccountId>;

	/// Convert the block number into a balance.
	type BlockNumberToBalance: Convert<Self::BlockNumber, BalanceOf<Self>>;

	/// The minimum amount transferred to call `vested_transfer`.
	type MinVestedTransfer: Get<BalanceOf<Self>>;

	/// Weight information for extrinsics in this pallet.
	type WeightInfo: WeightInfo;

	// type UnlockDuration: Get<Self::BlockNumber>;
}

const VESTING_ID: LockIdentifier = *b"vesting ";

/// Struct to encode the vesting schedule of an individual account.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct VestingInfo<Balance, BlockNumber> {
	/// Locked amount at genesis.
	pub locked: Balance,
	/// Amount that gets unlocked every block after `starting_block`.
	pub unlock_amount_of_per_duration: Balance,
	/// unlock duration
	pub block_number_of_per_duration: BlockNumber,
	/// Starting block for unlocking(vesting).
	pub starting_block: BlockNumber,
}

impl<
	Balance: AtLeast32BitUnsigned + Copy,
	BlockNumber: AtLeast32BitUnsigned + Copy,
> VestingInfo<Balance, BlockNumber> {
	/// Amount locked at block `n`.
	pub fn locked_at<
		BlockNumberToBalance: Convert<BlockNumber, Balance>
	>(&self, n: BlockNumber, duration: BlockNumber) -> Balance {
		// Number of blocks that count toward vesting
		// Saturating to 0 when n < starting_block

		let vested_block_count = n.saturating_sub(self.starting_block);

		// 超过一定周期 才能够执行减仓操作
		let num = vested_block_count.checked_div(&duration).unwrap_or(vested_block_count);

		let vested_block_count = BlockNumberToBalance::convert(num);
		// Return amount that is still locked in vesting
		let maybe_balance = vested_block_count.checked_mul(&self.unlock_amount_of_per_duration);

		if let Some(balance) = maybe_balance {
			self.locked.saturating_sub(balance)
		} else {
			Zero::zero()
		}
	}
}

decl_storage! {
	trait Store for Module<T: Config> as Vesting {
		/// Information regarding the vesting of a given account.
		pub Vesting get(fn vesting):
			map hasher(blake2_128_concat) T::AccountId
			=> Option<VestingInfo<BalanceOf<T>, T::BlockNumber>>;
	}
	add_extra_genesis {
		config(vesting): Vec<(T::AccountId, T::BlockNumber, T::BlockNumber, BalanceOf<T>)>;
		build(|config: &GenesisConfig<T>| {
			use sp_runtime::traits::Saturating;
			// Generate initial vesting configuration
			// * who - Account which we are generating vesting configuration for
			// * begin - Block when the account will start to vest
			// * length - Number of blocks from `begin` until fully vested
			// * liquid - Number of units which can be spent before vesting begins
			for &(ref who, begin, length, liquid) in config.vesting.iter() {
				let balance = T::Currency::free_balance(who);
				assert!(!balance.is_zero(), "Currencies must be init'd before vesting");
				// Total genesis `balance` minus `liquid` equals funds locked for vesting
				let locked = balance.saturating_sub(liquid);
				let length_as_balance = T::BlockNumberToBalance::convert(length);
				let per_block = locked / length_as_balance.max(sp_runtime::traits::One::one());

				Vesting::<T>::insert(who, VestingInfo {
					locked: locked,
					unlock_amount_of_per_duration: per_block,
					block_number_of_per_duration: T::BlockNumber::from(1u32),
					starting_block: begin
				});
				let reasons = WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE;
				T::Currency::set_lock(VESTING_ID, who, locked, reasons);
			}
		})
	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId, Balance = BalanceOf<T> {
		/// The amount vested has been updated. This could indicate more funds are available. The
		/// balance given is the amount which is left unvested (and thus locked).
		/// \[account, unvested\]
		VestingUpdated(AccountId, Balance),
		/// An \[account\] has become fully vested. No further vesting can happen.
		VestingCompleted(AccountId),
	}
);

decl_error! {
	/// Error for the vesting module.
	pub enum Error for Module<T: Config> {
		/// The account given is not vesting.
		NotVesting,
		/// An existing vesting schedule already exists for this account that cannot be clobbered.
		ExistingVestingSchedule,
		/// Amount being transferred is too low to create a vesting schedule.
		AmountLow,
		/// unlock duration should not be Zero
		UnlockDurationZero,
	}
}

decl_module! {
	/// Vesting module declaration.
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		/// The minimum amount to be transferred to create a new vesting schedule.
		const MinVestedTransfer: BalanceOf<T> = T::MinVestedTransfer::get();

		fn deposit_event() = default;

		/// Unlock any vested funds of the sender account.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must have funds still
		/// locked under this module.
		///
		/// Emits either `VestingCompleted` or `VestingUpdated`.
		///
		/// # <weight>
		/// - `O(1)`.
		/// - DbWeight: 2 Reads, 2 Writes
		///     - Reads: Vesting Storage, Balances Locks, [Sender Account]
		///     - Writes: Vesting Storage, Balances Locks, [Sender Account]
		/// # </weight>
		#[weight = T::WeightInfo::vest_locked(MaxLocksOf::<T>::get())
			.max(T::WeightInfo::vest_unlocked(MaxLocksOf::<T>::get()))
		]
		fn vest(origin) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::update_lock(who)
		}

		/// Unlock any vested funds of a `target` account.
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `target`: The account whose vested funds should be unlocked. Must have funds still
		/// locked under this module.
		///
		/// Emits either `VestingCompleted` or `VestingUpdated`.
		///
		/// # <weight>
		/// - `O(1)`.
		/// - DbWeight: 3 Reads, 3 Writes
		///     - Reads: Vesting Storage, Balances Locks, Target Account
		///     - Writes: Vesting Storage, Balances Locks, Target Account
		/// # </weight>
		#[weight = T::WeightInfo::vest_other_locked(MaxLocksOf::<T>::get())
			.max(T::WeightInfo::vest_other_unlocked(MaxLocksOf::<T>::get()))
		]
		fn vest_other(origin, target: <T::Lookup as StaticLookup>::Source) -> DispatchResult {
			ensure_signed(origin)?;
			Self::update_lock(T::Lookup::lookup(target)?)
		}

		/// Create a vested transfer.
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `target`: The account that should be transferred the vested funds.
		/// - `amount`: The amount of funds to transfer and will be vested.
		/// - `schedule`: The vesting schedule attached to the transfer.
		///
		/// Emits `VestingCreated`.
		///
		/// # <weight>
		/// - `O(1)`.
		/// - DbWeight: 3 Reads, 3 Writes
		///     - Reads: Vesting Storage, Balances Locks, Target Account, [Sender Account]
		///     - Writes: Vesting Storage, Balances Locks, Target Account, [Sender Account]
		/// # </weight>
		#[weight = T::WeightInfo::vested_transfer(MaxLocksOf::<T>::get())]
		pub fn vested_transfer(
			origin,
			target: <T::Lookup as StaticLookup>::Source,
			schedule: VestingInfo<BalanceOf<T>, T::BlockNumber>,
		) -> DispatchResult {
			let transactor = ensure_signed(origin)?;
			ensure!(schedule.locked >= T::MinVestedTransfer::get(), Error::<T>::AmountLow);

			ensure!(schedule.block_number_of_per_duration != T::BlockNumber::from(0u32), Error::<T>::UnlockDurationZero);

			let who = T::Lookup::lookup(target)?;
			ensure!(!Vesting::<T>::contains_key(&who), Error::<T>::ExistingVestingSchedule);

			T::Currency::transfer(&transactor, &who, schedule.locked, ExistenceRequirement::AllowDeath)?;

			Self::add_vesting_schedule(&who, schedule.locked, schedule.unlock_amount_of_per_duration, schedule.block_number_of_per_duration, schedule.starting_block)
				.expect("user does not have an existing vesting schedule; q.e.d.");

			Ok(())
		}

		/// Force a vested transfer.
		///
		/// The dispatch origin for this call must be _Root_.
		///
		/// - `source`: The account whose funds should be transferred.
		/// - `target`: The account that should be transferred the vested funds.
		/// - `amount`: The amount of funds to transfer and will be vested.
		/// - `schedule`: The vesting schedule attached to the transfer.
		///
		/// Emits `VestingCreated`.
		///
		/// # <weight>
		/// - `O(1)`.
		/// - DbWeight: 4 Reads, 4 Writes
		///     - Reads: Vesting Storage, Balances Locks, Target Account, Source Account
		///     - Writes: Vesting Storage, Balances Locks, Target Account, Source Account
		/// # </weight>
		#[weight = T::WeightInfo::force_vested_transfer(MaxLocksOf::<T>::get())]
		pub fn force_vested_transfer(
			origin,
			source: <T::Lookup as StaticLookup>::Source,
			target: <T::Lookup as StaticLookup>::Source,
			schedule: VestingInfo<BalanceOf<T>, T::BlockNumber>,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(schedule.locked >= T::MinVestedTransfer::get(), Error::<T>::AmountLow);

			ensure!(schedule.block_number_of_per_duration != T::BlockNumber::from(0u32), Error::<T>::UnlockDurationZero);

			let target = T::Lookup::lookup(target)?;
			let source = T::Lookup::lookup(source)?;
			ensure!(!Vesting::<T>::contains_key(&target), Error::<T>::ExistingVestingSchedule);

			T::Currency::transfer(&source, &target, schedule.locked, ExistenceRequirement::AllowDeath)?;

			Self::add_vesting_schedule(&target, schedule.locked, schedule.unlock_amount_of_per_duration, schedule.block_number_of_per_duration, schedule.starting_block)
				.expect("user does not have an existing vesting schedule; q.e.d.");

			Ok(())
		}
	}
}

impl<T: Config> Module<T> {
	/// (Re)set or remove the module's currency lock on `who`'s account in accordance with their
	/// current unvested amount.
	fn update_lock(who: T::AccountId) -> DispatchResult {
		let vesting = Self::vesting(&who).ok_or(Error::<T>::NotVesting)?;
		let now = <frame_system::Module<T>>::block_number();

		let locked_now = vesting.locked_at::<T::BlockNumberToBalance>(now, vesting.block_number_of_per_duration);

		if locked_now.is_zero() {
			T::Currency::remove_lock(VESTING_ID, &who);
			Vesting::<T>::remove(&who);
			Self::deposit_event(RawEvent::VestingCompleted(who));
		} else {
			let reasons = WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE;
			T::Currency::set_lock(VESTING_ID, &who, locked_now, reasons);
			Self::deposit_event(RawEvent::VestingUpdated(who, locked_now));
		}
		Ok(())
	}
}

impl<T: Config> VestingSchedule<T::AccountId> for Module<T> where
	BalanceOf<T>: MaybeSerializeDeserialize + Debug
{
	type Moment = T::BlockNumber;
	type Currency = T::Currency;

	/// Get the amount that is currently being vested and cannot be transferred out of this account.
	fn vesting_balance(who: &T::AccountId) -> Option<BalanceOf<T>> {
		if let Some(v) = Self::vesting(who) {
			let now = <frame_system::Module<T>>::block_number();
			let locked_now = v.locked_at::<T::BlockNumberToBalance>(now, v.block_number_of_per_duration);
			Some(T::Currency::free_balance(who).min(locked_now))
		} else {
			None
		}
	}

	/// Adds a vesting schedule to a given account.
	///
	/// If there already exists a vesting schedule for the given account, an `Err` is returned
	/// and nothing is updated.
	///
	/// On success, a linearly reducing amount of funds will be locked. In order to realise any
	/// reduction of the lock over time as it diminishes, the account owner must use `vest` or
	/// `vest_other`.
	///
	/// Is a no-op if the amount to be vested is zero.
	fn add_vesting_schedule(
		who: &T::AccountId,
		locked: BalanceOf<T>,
		unlock_amount_of_per_duration: BalanceOf<T>,
		block_number_of_per_duration: T::BlockNumber,
		starting_block: T::BlockNumber
	) -> DispatchResult {
		if locked.is_zero() { return Ok(()) }
		if Vesting::<T>::contains_key(who) {
			Err(Error::<T>::ExistingVestingSchedule)?
		}
		let vesting_schedule = VestingInfo {
			locked,
			unlock_amount_of_per_duration,
			block_number_of_per_duration,
			starting_block
		};
		Vesting::<T>::insert(who, vesting_schedule);
		// it can't fail, but even if somehow it did, we don't really care.
		let _ = Self::update_lock(who.clone());
		Ok(())
	}

	/// Remove a vesting schedule for a given account.
	fn remove_vesting_schedule(who: &T::AccountId) {
		Vesting::<T>::remove(who);
		// it can't fail, but even if somehow it did, we don't really care.
		let _ = Self::update_lock(who.clone());
	}
}

