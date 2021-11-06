use super::{Config as ListenConfig, Module};
use codec::{Decode, Encode};
use frame_system::{self as system};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::{prelude::*, result};

use listen_primitives::{constants::currency::*, Balance};

pub type SessionIndex = u32;
pub type RoomId = u64;

/// The cost of creating a group
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone, TypeInfo)]
pub struct CreateCost {
	pub Ten: Balance,
	pub Hundred: Balance,
	pub FiveHundred: Balance,
	pub TenThousand: Balance,
	pub NoLimit: Balance,
}

impl Default for CreateCost {
	fn default() -> Self {
		Self {
			Ten: 1 * UNIT,
			Hundred: 10 * UNIT,
			FiveHundred: 30 * UNIT,
			TenThousand: 200 * UNIT,
			NoLimit: 1000 * UNIT,
		}
	}
}

/// Time interval limits on dissolving the room.
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct DisbandTime<BlockNumber> {
	pub Ten: BlockNumber,
	pub Hundred: BlockNumber,
	pub FiveHundred: BlockNumber,
	pub TenThousand: BlockNumber,
	pub NoLimit: BlockNumber,
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct RemoveTime<BlockNumber> {
	pub Ten: BlockNumber,
	pub Hundred: BlockNumber,
	pub FiveHundred: BlockNumber,
	pub TenThousand: BlockNumber,
	pub NoLimit: BlockNumber,
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct PropsPrice<BalanceOf> {
	pub picture: BalanceOf,
	pub text: BalanceOf,
	pub video: BalanceOf,
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct AudioPrice<BalanceOf> {
	pub ten_seconds: BalanceOf,
	pub thirty_seconds: BalanceOf,
	pub minutes: BalanceOf,
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct AllProps {
	pub picture: u32,
	pub text: u32,
	pub video: u32,
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct RoomRewardInfo<Balance> {
	pub total_person: u32,
	pub already_get_count: u32,
	pub total_reward: Balance,
	pub already_get_reward: Balance,
	pub per_man_reward: Balance,
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct DisbandVote<BTreeSet, Balance> {
	pub approve_man: BTreeSet,
	pub reject_man: BTreeSet,
	pub approve_total_amount: Balance,
	pub reject_total_amount: Balance,
}

#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone, TypeInfo)]
pub struct RedPacket<AccountId, BTreeSet, Balance, BlockNumber, CurrencyId> {
	pub id: u128,
	pub currency_id: CurrencyId,
	pub boss: AccountId,
	pub total: Balance,
	pub lucky_man_number: u32,
	pub already_get_man: BTreeSet,
	pub min_amount_of_per_man: Balance,
	pub already_get_amount: Balance,
	pub end_time: BlockNumber,
}

#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone, TypeInfo)]
pub enum GroupMaxMembers {
	Ten,         // 10
	Hundred,     // 100
	FiveHundred, // 500
	TenThousand, // 1000010
	NoLimit,     //
}

impl GroupMaxMembers {
	pub fn into_u32(&self) -> result::Result<u32, &'static str> {
		match self {
			GroupMaxMembers::Ten => Ok(10u32),
			GroupMaxMembers::Hundred => Ok(100u32),
			GroupMaxMembers::FiveHundred => Ok(500u32),
			GroupMaxMembers::TenThousand => Ok(10_0000u32),
			GroupMaxMembers::NoLimit => Ok(u32::max_value()),
			_ => Err("There is no such room type"),
		}
	}
}

impl Default for GroupMaxMembers {
	fn default() -> Self {
		Self::Ten
	}
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct Audio {
	pub ten_seconds: u32,
	pub thirty_seconds: u32,
	pub minutes: u32,
}

#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone, TypeInfo)]
pub enum ListenVote {
	Approve,
	Reject,
}

#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone, TypeInfo)]
pub enum RewardStatus {
	Get,
	NotGet,
	Expire,
}

impl Default for RewardStatus {
	fn default() -> Self {
		Self::NotGet
	}
}

impl Default for ListenVote {
	fn default() -> Self {
		Self::Reject
	}
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct GroupInfo<
	AccountId,
	Balance,
	AllProps,
	Audio,
	BlockNumber,
	GroupMaxMembers,
	DisbandVote,
	Moment,
> {
	pub group_id: u64,
	pub create_payment: Balance,
	pub last_block_of_get_the_reward: BlockNumber,

	pub group_manager: AccountId,
	pub prime: Option<AccountId>,
	pub max_members: GroupMaxMembers,

	pub group_type: Vec<u8>,
	pub join_cost: Balance,
	pub props: AllProps,

	pub audio: Audio,

	pub total_balances: Balance,

	pub group_manager_balances: Balance,

	pub now_members_number: u32,

	pub last_remove_someone_block: BlockNumber,

	pub disband_vote_end_block: BlockNumber,

	pub disband_vote: DisbandVote,

	pub create_time: Moment,

	pub create_block: BlockNumber,

	pub consume: Vec<(AccountId, Balance)>,

	pub council: Vec<(AccountId, Balance)>,

	pub black_list: Vec<AccountId>,

	pub is_private: bool,
}

#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone, TypeInfo)]
pub struct PersonInfo<AllProps, Audio, Balance, RewardStatus> {
	pub props: AllProps,
	pub audio: Audio,
	pub cost: Balance,
	pub rooms: Vec<(RoomId, RewardStatus)>,
}

pub mod listen_time {

	pub mod remove {
		use listen_primitives::{constants::time::*, BlockNumber};
		pub const Ten: BlockNumber = 7 * DAYS;
		pub const Hundred: BlockNumber = 1 * DAYS;
		pub const FiveHundred: BlockNumber = 12 * HOURS;
		pub const TenThousand: BlockNumber = 8 * HOURS;
		pub const NoLimit: BlockNumber = 6 * HOURS;
	}

	pub mod disband {
		use listen_primitives::{constants::time::*, BlockNumber};
		pub const Ten: BlockNumber = 1 * DAYS;
		pub const FiveHundred: BlockNumber = 15 * DAYS;
		pub const Hundred: BlockNumber = 7 * DAYS;
		pub const TenThousand: BlockNumber = 30 * DAYS;
		pub const NoLimit: BlockNumber = 60 * DAYS;
	}
}

pub mod vote {
	pub const Pass: bool = true;
	pub const NotPass: bool = false;

	pub const End: bool = true;
	pub const NotEnd: bool = false;
}
