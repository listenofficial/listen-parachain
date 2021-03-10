/// A vesting schedule over a currency. This allows a particular currency to have vesting limits
/// applied to it.
///
use frame_support::traits::Currency;
use frame_support::dispatch::DispatchResult;

pub trait VestingSchedule<AccountId> {
	/// The quantity used to denote time; usually just a `BlockNumber`.
	type Moment;

	/// The currency that this schedule applies to.
	type Currency: Currency<AccountId>;

	/// Get the amount that is currently being vested and cannot be transferred out of this account.
	/// Returns `None` if the account has no vesting schedule.
	fn vesting_balance(who: &AccountId) -> Option<<Self::Currency as Currency<AccountId>>::Balance>;

	/// Adds a vesting schedule to a given account.
	///
	/// If there already exists a vesting schedule for the given account, an `Err` is returned
	/// and nothing is updated.
	///
	/// Is a no-op if the amount to be vested is zero.
	///
	/// NOTE: This doesn't alter the free balance of the account.
	fn add_vesting_schedule(
		who: &AccountId,
		locked: <Self::Currency as Currency<AccountId>>::Balance,
		per_block: <Self::Currency as Currency<AccountId>>::Balance,
		unlock_duration: Self::Moment,
		starting_block: Self::Moment,
	) -> DispatchResult;

	/// Remove a vesting schedule for a given account.
	///
	/// NOTE: This doesn't alter the free balance of the account.
	fn remove_vesting_schedule(who: &AccountId);
}
