use codec::{Encode, Decode};
use sp_runtime::RuntimeDebug;
use sp_std::{result, prelude::*};

pub type SessionIndex = u32;
pub type RoomId = u64;


use node_primitives::Balance;
use node_constants::currency::*;
/// 创建群的费用
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub struct CreateCost{
	pub Ten: Balance,
	pub Hundred: Balance,
	pub FiveHundred: Balance,
	pub TenThousand: Balance,
	pub NoLimit: Balance,
}

impl Default for CreateCost{
	fn default() -> Self{
		Self{
			Ten: 1*DOLLARS,
			Hundred: 10*DOLLARS,
			FiveHundred: 30*DOLLARS,
			TenThousand: 200*DOLLARS,
			NoLimit: 1000*DOLLARS,
		}
	}
}

/// 群解散的时间限制
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct DisbandTime<BlockNumber>{
	pub Ten: BlockNumber,
	pub Hundred: BlockNumber,
	pub FiveHundred: BlockNumber,
	pub TenThousand: BlockNumber,
	pub NoLimit: BlockNumber,
}


#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct RemoveTime<BlockNumber>{
	pub Ten: BlockNumber,
	pub Hundred: BlockNumber,
	pub FiveHundred: BlockNumber,
	pub TenThousand: BlockNumber,
	pub NoLimit: BlockNumber,
}



/// 道具的费用
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct PropsPrice<BalanceOf>{
	pub picture: BalanceOf,
	pub text: BalanceOf,
	pub video: BalanceOf,
}


/// 语音的费用
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct AudioPrice<BalanceOf>{
	pub ten_seconds: BalanceOf,
	pub thirty_seconds: BalanceOf,
	pub minutes: BalanceOf,
}


/// 所有道具的统计
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct AllProps{
	pub picture: u32,
	pub text: u32,
	pub video: u32,
}


/// 群的奖励信息
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct RoomRewardInfo<Balance>{
	pub total_person: u32,
	pub already_get_count: u32,
	pub total_reward: Balance,  // 总奖励
	pub already_get_reward: Balance, // 已经领取的奖励
	pub per_man_reward: Balance,  // 平均每个人的奖励

}


/// 解散投票
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct DisbandVote<BTreeSet>{
	pub approve_man: BTreeSet,
	pub reject_man: BTreeSet,
}




/// 红包
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub struct RedPacket<AccountId, BTreeSet, Balance, BlockNumber, CurrencyId>{

	pub id: u128, // 红包id
	pub currency_id: CurrencyId,
	pub boss: AccountId,  // 发红包的人
	pub total: Balance,	// 红包总金额
	pub lucky_man_number: u32, // 红包奖励的人数
	pub already_get_man: BTreeSet, // 已经领取红包的人
	pub min_amount_of_per_man: Balance, // 每个人领取的最小红包金额
	pub already_get_amount: Balance, // 已经总共领取的金额数
	pub end_time: BlockNumber, // 红包结束的时间

}


#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum GroupMaxMembers{
	Ten,  // 10人群
	Hundred, // 100人群
	FiveHundred, // 500人群
	TenThousand, // 1000010人群
	NoLimit,  // 不作限制
}

impl GroupMaxMembers{
	pub fn into_u32(&self) -> result::Result<u32, & 'static str>{
		match self{
			GroupMaxMembers::Ten => Ok(10u32),
			GroupMaxMembers::Hundred => Ok(100u32),
			GroupMaxMembers::FiveHundred => Ok(500u32),
			GroupMaxMembers::TenThousand => Ok(10_0000u32),
			GroupMaxMembers::NoLimit => Ok(u32::max_value()),
			_ => Err("群上限人数类型不匹配"),
		}

	}
}

impl Default for GroupMaxMembers{
	fn default() -> Self{
		Self::Ten
	}
}



/// 语音时长类型的统计
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct Audio{
	pub ten_seconds: u32,
	pub thirty_seconds: u32,
	pub minutes: u32,
}


/// 投票类型
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum VoteType{
	Approve,
	Reject,
}


// 个人在某房间的领取奖励的状态
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum RewardStatus{
	Get, // 已经领取
	NotGet,  // 还没有领取
	Expire, // 过期

}

// 默认状态未领取
impl Default for RewardStatus{
	fn default() -> Self{
		Self::NotGet
	}
}

// 默认不同意
impl Default for VoteType{
	fn default() -> Self{
		Self::Reject
	}
}

/// 邀请第三人进群的缴费方式
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum InvitePaymentType{
	inviter,  // 邀请人交费
	invitee,  // 被邀请人自己交
}

impl Default for InvitePaymentType {
	fn default() -> Self {
		Self::invitee
	}
}


/// 群的信息
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct GroupInfo<AccountId, Balance, AllProps, Audio, BlockNumber, GroupMaxMembers, DisbandVote, Moment>{
	pub group_id: u64,  // 群的id直接用自增的u64类型

	pub create_payment: Balance,  // 创建群时支付的费用

	pub last_block_of_get_the_reward: BlockNumber, // 群主上一次领取奖励的区块

	pub pledge_amount: Balance, // 群主抵押的费用

	pub group_manager: AccountId,  // 群主
	pub max_members: GroupMaxMembers, // 最大群人数

	pub group_type: Vec<u8>, // 群的类型（玩家自定义字符串）
	pub join_cost: Balance,  // 这个是累加的的还是啥？？？

	pub props: AllProps,  // 本群语音购买统计
	pub audio: Audio, // 本群道具购买统计

	pub total_balances: Balance, // 群总币余额
	pub group_manager_balances: Balance, // 群主币余额

	pub now_members_number: u32, // 目前群人数

	pub last_remove_height: BlockNumber,  // 群主上次踢人的高度
	pub last_disband_end_hight: BlockNumber,  // 上次解散群提议结束时的高度

	pub disband_vote: DisbandVote, // 投票信息
	pub this_disband_start_time: BlockNumber, // 解散议案开始投票的时间

	pub is_voting: bool,  // 是否出于投票状态
	pub create_time: Moment,

}


#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct PersonInfo<AllProps, Audio, Balance, RewardStatus>{
	pub props: AllProps, // 这个人的道具购买统计
	pub audio: Audio, // 这个人的语音购买统计
	pub cost: Balance, // 个人购买道具与语音的总费用
	pub rooms: Vec<(RoomId, RewardStatus)>,  // 这个人加入的所有房间
}


/// 听众类型
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum ListenerType{
	group_manager,  // 群主
	common,  // 普通听众
	honored_guest,  // 嘉宾
}

impl Default for ListenerType{
	fn default() -> Self{
		Self::common
	}
}


pub mod listen_time{

	// 踢人的时间限制
	pub mod remove {
		use node_primitives::BlockNumber;
		use node_constants::time::*;
		pub const Ten: BlockNumber = 7 * DAYS;
		pub const Hundred: BlockNumber = 1 * DAYS;
		pub const FiveHundred: BlockNumber = 12 * HOURS;
		pub const TenThousand: BlockNumber = 8 * HOURS;
		pub const NoLimit: BlockNumber = 6 * HOURS;
	}

	// 解散群的时间限制
	pub mod disband {
		use node_primitives::BlockNumber;
		use node_constants::time::*;
		pub const Ten: BlockNumber = 1 * DAYS;
		pub const FiveHundred: BlockNumber = 15 * DAYS;
		pub const Hundred: BlockNumber = 7 * DAYS;
		pub const TenThousand: BlockNumber = 30 * DAYS;
		pub const NoLimit: BlockNumber = 60 * DAYS;
	}
}

pub mod vote{
	pub const Pass: bool = true;
	pub const NotPass: bool = false;

	pub const End: bool = true;
	pub const NotEnd: bool = false;
}
