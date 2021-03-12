
#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]


pub mod raw;

pub use frame_support::{traits::{Get, Currency, ReservableCurrency, EnsureOrigin, ExistenceRequirement::KeepAlive, WithdrawReasons, OnUnbalanced, BalanceStatus as Status},
					debug, ensure, decl_module, decl_storage, decl_error, decl_event, weights::{Weight}, StorageValue, StorageMap, StorageDoubleMap, IterableStorageDoubleMap, Blake2_256};

use sp_std::{result, prelude::*, collections::btree_set::BTreeSet, collections::btree_map::BTreeMap, convert::TryFrom, cmp};

use frame_system::{self as system, ensure_signed, ensure_root};
use pallet_multisig;
use sp_runtime::{traits::{AccountIdConversion, Saturating, CheckedDiv, Zero}, DispatchResult, Percent, RuntimeDebug, ModuleId, traits::CheckedMul, DispatchError, SaturatedConversion};
use pallet_timestamp as timestamp;
use node_primitives::{Balance, AccountId, Tokens, CurrencyId};

use node_constants::{currency::*, time::*};

// use pallet_treasury as treasury;
use codec::{Encode, Decode};
use vote::*;

use listen_time::*;

use crate::raw::{RemoveTime, DisbandTime, listen_time, PropsPrice, AudioPrice, AllProps, RoomRewardInfo,
Audio, DisbandVote, RedPacket, GroupMaxMembers, VoteType, RewardStatus, InvitePaymentType, GroupInfo,
PersonInfo, vote, ListenerType, SessionIndex, RoomId, CreateCost};
use sp_std::convert::TryInto;

use orml_tokens;
use orml_traits::MultiCurrency;
use pallet_transfer;

pub(crate) type MultiBalanceOf<T> =
		<<T as pallet_transfer::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

pub(crate) type CurrencyIdOf<T> =
		<<T as pallet_transfer::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;

type BalanceOf<T> = <<T as pallet_transfer::Config>::NativeCurrency as Currency<<T as system::Config>::AccountId>>::Balance;
type PositiveImbalanceOf<T> = <<T as pallet_transfer::Config>::NativeCurrency as Currency<<T as frame_system::Config>::AccountId>>::PositiveImbalance;
type NegativeImbalanceOf<T> = <<T as pallet_transfer::Config>::NativeCurrency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

pub trait Config: system::Config + timestamp::Config + pallet_multisig::Config + pallet_transfer::Config {

	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	type Create: OnUnbalanced<PositiveImbalanceOf<Self>>;

	type ProposalRejection: OnUnbalanced<NegativeImbalanceOf<Self>>;

	type VoteExpire: Get<Self::BlockNumber>;

	type RedPacketMinAmount: Get<BalanceOf<Self>>;

	type RedPackExpire: Get<Self::BlockNumber>;

	type RewardDuration: Get<Self::BlockNumber>;

	type PledgeRate: Get<Percent>;

	type ManagerProportion: Get<Percent>;

	type RoomProportion: Get<Percent>;

	type ModuleId: Get<ModuleId>;


}


decl_storage! {
	trait Store for Module<T: Config> as ListenModule {

		/// 已经空投过的名单
		pub AlreadyAirDropList get(fn alreadly_air_drop_list): BTreeSet<T::AccountId>;

		/// 自增的group_id
		pub GroupId get(fn group_id): u64 = 1; // 初始化值是1

 		/// 创建群的费用
		pub CreatePayment get(fn create_cost): CreateCost;

		/// 自增的红包id
		pub RedPacketId get(fn red_packet_id): u128 = 1;

		/// 全网创建的所有群 (group_id => group_info)
		pub AllRoom get(fn all_room): map hasher(blake2_128_concat) u64 => Option<GroupInfo<T::AccountId, BalanceOf<T>,
		AllProps, Audio, T::BlockNumber, GroupMaxMembers, DisbandVote<BTreeSet<T::AccountId>>, T::Moment>>;

		/// 群里的所有人
		pub ListenersOfRoom get(fn listeners_of_room): map hasher(blake2_128_concat) u64 => BTreeSet<T::AccountId>;

		/// 所有人员的信息(购买道具, 购买语音, 以及加入的群)
		pub AllListeners get(fn all_listeners): map hasher(blake2_128_concat) T::AccountId => PersonInfo<AllProps, Audio, BalanceOf<T>, RewardStatus>;

		/// 解散的群的信息（用于解散后奖励) (session_id, room_id => 房间奖励信息)
		pub InfoOfDisbandRoom get(fn info_of_disband_room): double_map hasher(blake2_128_concat) SessionIndex, hasher(blake2_128_concat) u64 => RoomRewardInfo<BalanceOf<T>>;

		/// 有奖励数据的所有session
		pub AllSessionIndex get(fn all_session): Vec<SessionIndex>;

		/// 对应房间的所有红包 (room_id, red_packet_id, RedPacket)
		pub RedPacketOfRoom get(fn red_packets_of_room): double_map hasher(blake2_128_concat) u64, hasher(blake2_128_concat) u128 =>
		Option<RedPacket<T::AccountId, BTreeSet<T::AccountId>, BalanceOf<T>, T::BlockNumber,  CurrencyIdOf<T>>>;

		/// 多签账号的信息 （[参与多签的人员id] 阀值 多签账号）
		pub Multisig get(fn multisig): Option<(Vec<T::AccountId>, u16, T::AccountId)>;

		/// 踢人的时间限制
		pub RemoveInterval get(fn kick_time_limit): RemoveTime<T::BlockNumber> = RemoveTime{
			Ten: T::BlockNumber::from(remove::Ten),
			Hundred: T::BlockNumber::from(remove::Hundred),
			FiveHundred: T::BlockNumber::from(remove::FiveHundred),
			TenThousand: T::BlockNumber::from(remove::TenThousand),
			NoLimit: T::BlockNumber::from(remove::NoLimit),
		};

		/// 解散群的时间限制
		pub DisbandInterval get(fn disband_time_limit): DisbandTime<T::BlockNumber> = DisbandTime{
			Ten: T::BlockNumber::from(disband::Ten),
			Hundred: T::BlockNumber::from(disband::Hundred),
			FiveHundred: T::BlockNumber::from(disband::FiveHundred),
			TenThousand: T::BlockNumber::from(disband::TenThousand),
			NoLimit: T::BlockNumber::from(disband::NoLimit),
		};

		/// 道具的费用
		pub PropsPayment get(fn props_payment): PropsPrice<BalanceOf<T>> = PropsPrice{
			picture: <BalanceOf<T> as TryFrom::<Balance>>::try_from(Percent::from_percent(3) * DOLLARS).ok().unwrap(),
			text: <BalanceOf<T> as TryFrom::<Balance>>::try_from(Percent::from_percent(1) * DOLLARS).ok().unwrap(),
			video: <BalanceOf<T> as TryFrom::<Balance>>::try_from(Percent::from_percent(3) * DOLLARS).ok().unwrap(),
		};

		/// 语音的费用
		pub AudioPayment get(fn audio_payment): AudioPrice<BalanceOf<T>> = AudioPrice{
			ten_seconds: <BalanceOf<T> as TryFrom::<Balance>>::try_from(Percent::from_percent(1) * DOLLARS).ok().unwrap(),
			thirty_seconds: <BalanceOf<T> as TryFrom::<Balance>>::try_from(Percent::from_percent(2) * DOLLARS).ok().unwrap(),
			minutes: <BalanceOf<T> as TryFrom::<Balance>>::try_from(Percent::from_percent(2) * DOLLARS).ok().unwrap(),
		};

		/// 服务器id（用来领取红包)
		pub ServerId get(fn server_id): Option<T::AccountId>;

	}
}


decl_error! {
	/// Error for the elections module.
	pub enum Error for Module<T: Config> {
		/// 队列为空（没有人)
		VecEmpty,
		/// 阀值错误
		ThreshouldErr,
		/// 已经空投过
		AlreadyAirDrop,
		/// 创建群支付金额错误
		CreatePaymentErr,
		/// 房间不存在
		RoomNotExists,
		/// 房间里没有任何人
		RoomEmpty,
		/// 已经邀请此人
		AlreadyInvited,
		/// 自由余额金额不足以抵押
		BondTooLow,
		/// 是自己
		IsYourSelf,
		/// 自由余额不足
		FreeAmountNotEnough,
		/// 数据溢出
		Overflow,
		/// 已经在群里
		InRoom,
		/// 没有被邀请过
		NotInvited,
		/// 数据转换错误
		ConvertErr,
		/// 不是群主
		NotManager,
		/// 不在群里
		NotInRoom,
		/// 权限错误
		PermissionErr,
		/// 没有到踢人的时间
		NotRemoveTime,
		/// 正在投票
		IsVoting,
		/// 没有在投票
		NotVoting,
		/// 重复投票
		RepeatVote,
		/// 没有到解散群提议的时间
		NotUntilDisbandTime,
		/// 没有加入任何房间
		NotIntoAnyRoom,
		/// 金额太小
		AmountTooLow,
		/// 红包不存在
		RedPacketNotExists,
		/// 余额不足
		AmountNotEnough,
		/// 领取红包人数达到上线
		ToMaxNumber,
		/// 次数错误
		CountErr,
		/// 过期
		Expire,
		/// 不是多签id
		NotMultisigId,
		/// 多签id还没有设置
		MultisigIdIsNone,
		/// 邀请你自己
		InviteYourself,
		/// 必须有付费类型
		MustHavePaymentType,
		/// 非法金额（房间费用与上次相同)
		InVailAmount,
		/// 群人数达到上限
		MembersNumberToMax,
		/// 未知的群类型
		UnknownRoomType,
		/// 成员重复
		MemberDuplicate,
		/// 服务器id没有设置
		ServerIdNotExists,
		/// 不是服务器的id
		NotServerId,
		/// 没有 这个代币
		TokenErr,
		/// 除数是0
		DivZero,
		/// 群主已经领取奖励
		AlreadyReward,
}}



decl_module! {

	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		// Initializing events

		/// 空投一次投多少token
		const AirDropAmount: BalanceOf<T> = T::AirDropAmount::get();
		/// 解散群提案可以存在多长时间
		const VoteExpire: T::BlockNumber = T::VoteExpire::get();
		/// 每个人原则上领取的最小红包金额
		const RedPacketMinAmount: BalanceOf<T> = T::RedPacketMinAmount::get();
		/// 红包存在多长时间
		const RedPackExpire: T::BlockNumber = T::RedPackExpire::get();
		/// 奖励群主的周期(多久奖励群主一次)
		const RewardDuration: T::BlockNumber = T::RewardDuration::get();
		/// 群主抵押的利率
		const PledgeRate: Percent = T::PledgeRate::get();
		/// 群主领取的群资产的比例（按照周期领取)
		const ManagerProportion: Percent = T::ManagerProportion::get();
		/// 给群生成新资产的比例（按照周期)
		const RoomProportion: Percent = T::RoomProportion::get();
		/// The treasury's module id, used for deriving its sovereign account ID.
		const ModuleId: ModuleId = T::ModuleId::get();


		type Error = Error<T>;
		fn deposit_event() = default;


		/// 设置用于空投的多签
		#[weight = 10_000]
		fn set_multisig(origin, who: Vec<T::AccountId>, threshould: u16){
			ensure_root(origin)?;
			let who = Self::sort_account_id(who)?;

			ensure!(threshould > 0u16 && threshould <= who.clone().len() as u16, Error::<T>::ThreshouldErr);

			let multisig_id = <pallet_multisig::Module<T>>::multi_account_id(&who, threshould.clone());
			<Multisig<T>>::put((who, threshould, multisig_id));

			Self::deposit_event(RawEvent::SetMultisig);

		}

		/// 设置服务器id(用来代领红包)
		#[weight = 10_000]
		fn set_server_id(origin, account_id: T::AccountId) {
			let who = ensure_signed(origin)?;
			//// 获取多签账号id
			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigIdIsNone)?;

			/// 是多签账号才给执行
			ensure!(who.clone() == multisig_id.clone(), Error::<T>::NotMultisigId);

			<ServerId<T>>::put(account_id.clone());

			Self::deposit_event(RawEvent::SetServerId(account_id));

		}


		/// 空投
		#[weight = 10_000]
		fn air_drop(origin, des: T::AccountId) -> DispatchResult {
			/// 执行空投的账号
			let who = ensure_signed(origin)?;

			/// 获取多签账号id
			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigIdIsNone)?;

			/// 是多签账号才给执行
			ensure!(who.clone() == multisig_id.clone(), Error::<T>::NotMultisigId);

			/// 已经空投过的不能再操作
			ensure!(!<AlreadyAirDropList<T>>::get().contains(&des), Error::<T>::AlreadyAirDrop);

			/// fixme 国库向空投的目标账号转账 0.99(前提是国库必须先有资金)
			let from = Self::treasury_id();
			T::NativeCurrency::transfer(&from, &des, T::AirDropAmount::get(), KeepAlive)?;

			// 添加空投记录
			<AlreadyAirDropList<T>>::mutate(|h| h.insert(des.clone()));
			<system::Module<T>>::inc_ref(&des);

			Self::deposit_event(RawEvent::AirDroped(who, des));
			Ok(())

		}


		/// 创建群
		#[weight = 10_000]
		fn create_room(origin, max_members: GroupMaxMembers, group_type: Vec<u8>, join_cost: BalanceOf<T>, pledge: Option<BalanceOf<T>>) -> DispatchResult{
			let who = ensure_signed(origin)?;

			let pledge = match pledge {
				Some(x) => x,
				None => <BalanceOf<T>>::from(0u32),
			};

			let create_cost = Self::create_cost();

			let create_payment: Balance = match max_members.clone(){
				GroupMaxMembers::Ten => create_cost.Ten,
				GroupMaxMembers::Hundred => create_cost.Hundred,
				GroupMaxMembers::FiveHundred => create_cost.FiveHundred,
				GroupMaxMembers::TenThousand => create_cost.TenThousand,
				GroupMaxMembers::NoLimit => create_cost.NoLimit,
				_ => return Err(Error::<T>::UnknownRoomType)?,
			};
			let create_payment = < BalanceOf<T> as TryFrom::<Balance>>::try_from(create_payment)
			.map_err(|_| Error::<T>::CreatePaymentErr)?;

			/// 查看群主的余额是否足够
			let reasons = WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE;
			let free_balance = T::NativeCurrency::free_balance(&who);
			let new_balance = free_balance.saturating_sub(create_payment + pledge);
			T::NativeCurrency::ensure_can_withdraw(&who, <BalanceOf<T>>::from(10u32), reasons, new_balance)?;

			// 群主把创建群的费用直接打到国库
			let to = Self::treasury_id();
			T::NativeCurrency::transfer(&who, &to, create_payment.clone(), KeepAlive)?;

			//群主把抵押的费用转到自己的抵押账户中
			T::NativeCurrency::repatriate_reserved(&who, &who, pledge, Status::Reserved)?;

			let group_id = <GroupId>::get();
			let group_info = GroupInfo{
				group_id: group_id,
				create_payment: create_payment,
				last_block_of_get_the_reward: Self::now(),
				pledge_amount: pledge,
				group_manager: who.clone(),
				max_members: max_members,
				group_type: group_type,
				join_cost: join_cost,
				props: AllProps::default(),
				audio: Audio::default(),
				total_balances: <BalanceOf<T>>::from(0u32),
				group_manager_balances: <BalanceOf<T>>::from(0u32),
				now_members_number: 1u32,
				last_remove_height: T::BlockNumber::default(),
				last_disband_end_hight: T::BlockNumber::default(),
				disband_vote: DisbandVote::default(),
				this_disband_start_time: T::BlockNumber::default(),
				is_voting: false,
				create_time: <timestamp::Module<T>>::get(),
			};

			<AllRoom<T>>::insert(group_id, group_info);
			<AllListeners<T>>::mutate(who.clone(), |h| h.rooms.push((group_id, RewardStatus::default())));
			<ListenersOfRoom<T>>::mutate(group_id, |h| h.insert(who.clone()));
			<GroupId>::mutate(|h| *h += 1);

			Self::deposit_event(RawEvent::CreatedRoom(who, group_id));

			Ok(())
		}


		/// 群主领取自己的奖励(一部分是抵押的奖励， 一部分是群资产产生的利息)
		#[weight = 10_000]
		fn manager_get_reward(origin, group_id: u64) {
			let who = ensure_signed(origin)?;
			let room_info = <AllRoom<T>>::get(group_id);
			/// 群存在
			ensure!(room_info.is_some(), Error::<T>::RoomNotExists);
			let mut room_info = room_info.unwrap();
			/// 是群主
			ensure!(who.clone() == room_info.group_manager.clone(), Error::<T>::NotManager);

			/// 获取群主上一次领取奖励的高度
			let last_block = room_info.last_block_of_get_the_reward.clone();
			let now = Self::now();
			let time = now.saturating_sub(last_block);
			let duration_num = time.checked_div(&T::RewardDuration::get()).ok_or(Error::<T>::DivZero)?;
			/// 计算真实的领取奖励的区块
			let real_this_block = last_block.saturating_add(duration_num * T::RewardDuration::get());

			if duration_num.is_zero() {
				return Err(Error::<T>::AlreadyReward)?;
			}
			else {
				// 领取抵押币的奖励
				let pledge = room_info.pledge_amount.clone();
				let reward = T::PledgeRate::get() * pledge;
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&who, reward));

				// 领取群资产产生的利息
				let room_total_amount = room_info.total_balances.clone();
				let manager_proportion_amount = T::ManagerProportion::get() * room_total_amount;
				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&who, manager_proportion_amount));

				// 群资产按照比例增加
				let room_add = T::RoomProportion::get() * room_total_amount;

				// 更新群信息
				room_info.last_block_of_get_the_reward = real_this_block;
				room_info.total_balances = room_info.total_balances.clone().saturating_add(room_add);
				<AllRoom<T>>::insert(group_id, room_info);
				Self::deposit_event(RawEvent::ManagerGetReward(who,  reward + manager_proportion_amount, room_add));

			}
		}


		/// 群主修改进群的费用
		#[weight = 10_000]
		fn update_join_cost(origin, group_id: u64, join_cost: BalanceOf<T>) -> DispatchResult{
			let who = ensure_signed(origin)?;
			let room_info = <AllRoom<T>>::get(group_id);
			// 群存在
			ensure!(room_info.is_some(), Error::<T>::RoomNotExists);
			let mut room_info = room_info.unwrap();
			// 是群主
			ensure!(who.clone() == room_info.group_manager.clone(), Error::<T>::NotManager);
			// 金额不能与原先的相同
			ensure!(room_info.join_cost.clone() != join_cost.clone(),  Error::<T>::InVailAmount);
			room_info.join_cost = join_cost.clone();
			<AllRoom<T>>::insert(group_id, room_info);
			Self::deposit_event(RawEvent::JoinCostChanged(group_id, join_cost));
			Ok(())
		}


		/// 进群
		#[weight = 10_000]
		fn into_room(origin, group_id: u64, invite: T::AccountId, inviter: Option<T::AccountId>, payment_type: Option<InvitePaymentType>) -> DispatchResult{
			let who = ensure_signed(origin)?;

			/// 获取多签账号id
			let (_, _, multisig_id) = <Multisig<T>>::get().ok_or(Error::<T>::MultisigIdIsNone)?;
			/// 是多签账号才给执行
			ensure!(who.clone() == multisig_id.clone(), Error::<T>::NotMultisigId);

			if inviter.is_some() {
				// 被邀请人与邀请人不能相同
				ensure!(inviter.clone().unwrap() != invite.clone(), Error::<T>::IsYourSelf);
				// 邀请别人必须要选择付费类型
				ensure!(payment_type.is_some(), Error::<T>::MustHavePaymentType);
			}

			let room_info = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;

 			// 如果进群人数已经达到上限， 不能进群
			ensure!(room_info.max_members.clone().into_u32()? >= room_info.now_members_number.clone(), Error::<T>::MembersNumberToMax);

			// 如果自己已经在群里 不需要重新进
			ensure!(!(Self::is_in_room(group_id, invite.clone())?), Error::<T>::InRoom);

			Self::join_do(invite.clone(), group_id, inviter.clone(), payment_type.clone())?;

			Self::deposit_event(RawEvent::IntoRoom(invite, inviter, group_id));
			Ok(())
		}


		/// 在群里购买道具
		#[weight = 10_000]
		fn buy_props_in_room(origin, group_id: u64, props: AllProps) -> DispatchResult{
			let who = ensure_signed(origin)?;

			/// 自己在群里
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);

			/// 计算道具总费用
			let mut dollars = <BalanceOf<T>>::from(0u32);

			let props_cost = <PropsPayment<T>>::get();
			if props.picture > 0u32{
				dollars = props_cost.picture * <BalanceOf<T>>::from(props.picture);
			}
			if props.text > 0u32{
				dollars += props_cost.text * <BalanceOf<T>>::from(props.text);
			}
			if props.video > 0u32{
				dollars += props_cost.video * <BalanceOf<T>>::from(props.video);
			}

			/// 把U128转换成balance
			let cost = dollars;

			/// ********以上数据不需要额外处理 不可能出现panic*************

			/// 扣除费用
			T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(&who, cost.clone(), WithdrawReasons::TRANSFER.into(), KeepAlive)?);

			// 修改群信息
			let mut room = <AllRoom<T>>::get(group_id).unwrap();
			room.props.picture = room.props.picture.checked_add(props.picture).ok_or(Error::<T>::Overflow)?;
			room.props.text = room.props.text.checked_add(props.text).ok_or(Error::<T>::Overflow)?;
			room.props.video = room.props.video.checked_add(props.video).ok_or(Error::<T>::Overflow)?;

			room.total_balances += cost.clone();

			<AllRoom<T>>::insert(group_id, room);

			// 修改个人信息
			let mut person = <AllListeners<T>>::get(who.clone());
			person.props.picture = person.props.picture.checked_add(props.picture).ok_or(Error::<T>::Overflow)?;
			person.props.text = person.props.text.checked_add(props.text).ok_or(Error::<T>::Overflow)?;
			person.props.video = person.props.video.checked_add(props.video).ok_or(Error::<T>::Overflow)?;
			person.cost += cost.clone();

			<AllListeners<T>>::insert(who.clone(), person);

			Self::deposit_event(RawEvent::BuyProps(who));
			Ok(())

		}


		/// 在群里购买语音
		#[weight = 10_000]
		fn buy_audio_in_room(origin, group_id: u64, audio: Audio) -> DispatchResult{
			let who = ensure_signed(origin)?;

			// 自己在群里
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);

			// 计算道具总费用
			let mut dollars = <BalanceOf<T>>::from(0u32);

			let audio_cost = <AudioPayment<T>>::get();
			if audio.ten_seconds > 0u32{
				dollars = audio_cost.ten_seconds * <BalanceOf<T>>::from(audio.ten_seconds);
			}
			if audio.thirty_seconds > 0u32{
				dollars += audio_cost.thirty_seconds * <BalanceOf<T>>::from(audio.thirty_seconds);
			}
			if audio.minutes > 0u32{
				dollars += audio_cost.minutes * <BalanceOf<T>>::from(audio.minutes);
			}

			// 把U128转换成balance
			let cost = dollars;
			// ********以上数据不需要额外处理 不可能出现panic*************

			// 扣除费用
			T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(&who, cost.clone(), WithdrawReasons::TRANSFER.into(), KeepAlive)?);

			// 修改群信息
			let mut room = <AllRoom<T>>::get(group_id).unwrap();
			room.audio.ten_seconds = room.audio.ten_seconds.checked_add(audio.ten_seconds).ok_or(Error::<T>::Overflow)?;
			room.audio.thirty_seconds = room.audio.thirty_seconds.checked_add(audio.thirty_seconds).ok_or(Error::<T>::Overflow)?;
			room.audio.minutes = room.audio.minutes.checked_add(audio.minutes).ok_or(Error::<T>::Overflow)?;

			room.total_balances += cost.clone();
			<AllRoom<T>>::insert(group_id, room);

			// 修改个人信息
			let mut person = <AllListeners<T>>::get(who.clone());
			person.audio.ten_seconds = person.audio.ten_seconds.checked_add(audio.ten_seconds).ok_or(Error::<T>::Overflow)?;
			person.audio.thirty_seconds = person.audio.thirty_seconds.checked_add(audio.thirty_seconds).ok_or(Error::<T>::Overflow)?;
			person.audio.minutes = person.audio.minutes.checked_add(audio.minutes).ok_or(Error::<T>::Overflow)?;
			person.cost += cost.clone();

			<AllListeners<T>>::insert(who.clone(), person);

			Self::deposit_event(RawEvent::BuyAudio(who));

			Ok(())

		}


		/// 设置创建群需要的费用
		#[weight = 10_000]
		fn set_create_cost(origin, max_members: GroupMaxMembers, amount: Balance) {
			ensure_root(origin)?;

			match max_members {
				GroupMaxMembers::Ten => CreatePayment::mutate(|h| h.Ten = amount),
				GroupMaxMembers::Hundred => CreatePayment::mutate(|h| h.Hundred = amount),
				GroupMaxMembers::FiveHundred => CreatePayment::mutate(|h| h.FiveHundred = amount),
				GroupMaxMembers::TenThousand => CreatePayment::mutate(|h| h.TenThousand = amount),
				GroupMaxMembers::NoLimit => CreatePayment::mutate(|h| h.NoLimit = amount),
				_ => return Err(Error::<T>::UnknownRoomType)?,
			}

			Self::deposit_event(RawEvent::SetCreateCost);

		}


		/// 群主踢人
		#[weight = 10_000]
		fn remove_someone(origin, group_id: u64, who: T::AccountId) -> DispatchResult {
			let manager = ensure_signed(origin)?;

			let mut room = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
			// 是群主
			ensure!(room.group_manager == manager.clone(), Error::<T>::NotManager);
			// 这个人在群里
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);

			let now = Self::now();

			if room.last_remove_height > T::BlockNumber::from(0u32){
				let until = now - room.last_remove_height;

				match room.max_members	{

					GroupMaxMembers::Ten => {
						if until <= T::BlockNumber::from(remove::Ten){
							return Err(Error::<T>::NotRemoveTime)?;
						}
					},

					GroupMaxMembers::Hundred => {
						if until <= T::BlockNumber::from(remove::Hundred){
							return Err(Error::<T>::NotRemoveTime)?;
						}
					},

					GroupMaxMembers::FiveHundred => {
						if until <= T::BlockNumber::from(remove::FiveHundred){
							return Err(Error::<T>::NotRemoveTime)?;
						}
					},

					GroupMaxMembers::TenThousand => {
						if until <= T::BlockNumber::from(remove::TenThousand){
							return Err(Error::<T>::NotRemoveTime)?;
						}
					},

					GroupMaxMembers::NoLimit => {
						if until <= T::BlockNumber::from(remove::NoLimit){
							return Err(Error::<T>::NotRemoveTime)?;
						}
					}

				}

			}

			// 修改数据
			<AllListeners<T>>::mutate(who.clone(), |h| h.rooms.retain(|x| x.0 != group_id.clone()));
			<ListenersOfRoom<T>>::mutate(group_id, |h| h.remove(&who));
			room.now_members_number = room.now_members_number.checked_sub(1u32).ok_or(Error::<T>::Overflow)?;
			room.last_remove_height = now;
			<AllRoom<T>>::insert(group_id, room);

			Self::deposit_event(RawEvent::Kicked(who.clone(), group_id));

			Ok(())

		}


		/// 群员要求解散群
		#[weight = 10_000]
		fn ask_for_disband_room(origin, group_id: u64) -> DispatchResult{
			let who = ensure_signed(origin)?;
			// 这个人是群成员
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);

			let mut room = <AllRoom<T>>::get(group_id).unwrap();

			let now = Self::now();

			// 如果有上一次解散记录
			if room.last_disband_end_hight > T::BlockNumber::from(0u32){
				let until = now.clone() - room.last_disband_end_hight;

				match room.max_members	{
					GroupMaxMembers::Ten => {
						if until <= T::BlockNumber::from(disband::Ten){
							return Err(Error::<T>::NotUntilDisbandTime)?;
						}
					},

					GroupMaxMembers::Hundred => {
						if until <= T::BlockNumber::from(disband::Hundred){
							return Err(Error::<T>::NotUntilDisbandTime)?;
						}
					},

					GroupMaxMembers::FiveHundred => {
						if until <= T::BlockNumber::from(disband::FiveHundred){
							return Err(Error::<T>::NotUntilDisbandTime)?;
						}
					},

					GroupMaxMembers::TenThousand => {
						if until <= T::BlockNumber::from(disband::TenThousand){
							return Err(Error::<T>::NotUntilDisbandTime)?;
						}
					},

					GroupMaxMembers::NoLimit => {
						if until <= T::BlockNumber::from(disband::NoLimit){
							return Err(Error::<T>::NotUntilDisbandTime)?;
						}
					}

				}
			}

			// 该群还未处于投票状态
			ensure!(!room.is_voting.clone(), Error::<T>::IsVoting);

			/// 转创建群时费用的1/10转到国库(这个费用好像已经全部转到国库了)
			let disband_payment = Percent::from_percent(10) * room.create_payment.clone();
			let to = Self::treasury_id();
			T::NativeCurrency::transfer(&who, &to, disband_payment, KeepAlive)?;

			room.is_voting = true;
			room.this_disband_start_time = Self::now();

			// 自己申请的 算自己赞成一票
			room.disband_vote.approve_man.insert(who.clone());
			<AllRoom<T>>::insert(group_id, room);

			Self::deposit_event(RawEvent::AskForDisband(who.clone(), group_id));
			Ok(())
		}


		/// 设置语音单价
		#[weight = 10_000]
		fn set_audio_price(origin, cost: AudioPrice<BalanceOf<T>>){
			ensure_root(origin)?;
			<AudioPayment<T>>::put(cost);
			Self::deposit_event(RawEvent::SetAudioPrice);
		}


		/// 设置道具单价
		#[weight = 10_000]
		fn set_props_price(origin, cost: PropsPrice<BalanceOf<T>>){
			ensure_root(origin)?;
			<PropsPayment<T>>::put(cost);
			Self::deposit_event(RawEvent::SetPropsPrice);
		}


		/// 设置群主踢人的时间间隔
		#[weight = 10_000]
		fn set_remove_interval(origin, time: RemoveTime<T::BlockNumber>){
			ensure_root(origin)?;
			<RemoveInterval<T>>::put(time);
			Self::deposit_event(RawEvent::SetKickInterval);
		}


		/// 设置解散群的时间间隔
		#[weight = 10_000]
		fn set_disband_interval(origin, time: DisbandTime<T::BlockNumber>){
			ensure_root(origin)?;
			<DisbandInterval<T>>::put(time);
			Self::deposit_event(RawEvent::SetDisbandInterval);
		}


		/// 给解散群的提案投票
		#[weight = 10_000]
		fn vote(origin, group_id: u64, vote: VoteType) -> DispatchResult{
			let who = ensure_signed(origin)?;

			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);

			let mut room = <AllRoom<T>>::get(group_id).unwrap();

			// 正在投票
			ensure!(room.is_voting, Error::<T>::NotVoting);

			let now = Self::now();

			// 不能二次投票
			match vote {
				// 如果投的是赞同票
				VoteType::Approve => {
					if room.disband_vote.approve_man.get(&who).is_some(){
						return Err(Error::<T>::RepeatVote)?;
					}
					room.disband_vote.approve_man.insert(who.clone());
					room.disband_vote.reject_man.remove(&who);

				},
				VoteType::Reject => {
					if room.disband_vote.reject_man.get(&who).is_some(){
						return Err(Error::<T>::RepeatVote)?;
					}
					room.disband_vote.reject_man.insert(who.clone());
					room.disband_vote.approve_man.remove(&who);

				},
				}

			<AllRoom<T>>::insert(group_id, room.clone());

			// 如果结束  就进行下一步
			let vote_result = Self::is_vote_end(room.now_members_number.clone(), room.disband_vote.clone(), room.this_disband_start_time);
			if vote_result.0 == End{
				// 如果是通过 那么就删除房间信息跟投票信息 添加投票结果信息
				if vote_result.1 == Pass{

					// 先解决红包(剩余红包归还给发红包的人)
					Self::remove_redpacket_by_room_id(group_id, true);

					let cur_session = Self::get_session_index();
					let mut session_indexs = <AllSessionIndex>::get();
					if session_indexs.is_empty(){
						session_indexs.push(cur_session)
					}
					else{
						let len = session_indexs.clone().len();
						// 获取最后一个数据
						let last = session_indexs.swap_remove(len - 1);
						if last != cur_session{
							session_indexs.push(last);
						}
						session_indexs.push(cur_session);
					}
					<AllSessionIndex>::put(session_indexs);

					let total_reward = room.total_balances.clone();
					let manager_reward = room.group_manager_balances.clone();
					// 把属于群主的那部分给群主
					T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&room.group_manager, manager_reward));

					let listener_reward = total_reward.clone() - manager_reward.clone();
					let session_index = Self::get_session_index();
					let per_man_reward = listener_reward.clone() / <BalanceOf<T>>::from(room.now_members_number);
					let room_rewad_info = RoomRewardInfo{
						total_person: room.now_members_number.clone(),
						already_get_count: 0u32,
						total_reward: listener_reward.clone(),
						already_get_reward: <BalanceOf<T>>::from(0u32),
						per_man_reward: per_man_reward.clone(),
					};

					<InfoOfDisbandRoom<T>>::insert(session_index, group_id, room_rewad_info);

					<AllRoom<T>>::remove(group_id);

				}

				// 如果是不通过 那么就删除投票信息 回到投票之前的状态
				else{
					// 删除有关投票信息
					let last_disband_end_hight = now.clone();
					room.last_disband_end_hight = last_disband_end_hight;
					room.is_voting = false;
					room.this_disband_start_time = <T::BlockNumber>::from(0u32);
					room.disband_vote.approve_man = BTreeSet::<T::AccountId>::new();
					room.disband_vote.reject_man = BTreeSet::<T::AccountId>::new();

					<AllRoom<T>>::insert(group_id, room);
				}

			}
			Self::deposit_event(RawEvent::DisbandVote(who.clone(), group_id));
			Ok(())

		}


		/// 一键领取自己的所有奖励
		#[weight = 10_000]
		fn pay_out(origin) -> DispatchResult {
			// 未领取奖励的有三种可能 一种是群没有解散 一种是群解散了未领取 一种是过期了但是还没有打过期标签
			let who = ensure_signed(origin)?;
			let mut amount = <BalanceOf<T>>::from(0u32);
			// 一定要有加入的房间
			ensure!(<AllListeners<T>>::contains_key(who.clone()) && !<AllListeners<T>>::get(who.clone()).rooms.is_empty(), Error::<T>::NotIntoAnyRoom);
			let rooms = <AllListeners<T>>::get(who.clone()).rooms;

			let mut new_rooms = rooms.clone();

			for room in rooms.iter(){
				// 还没有领取才会去操作
				if room.1 == RewardStatus::NotGet{

					// 已经进入待奖励队列
					if !<AllRoom<T>>::contains_key(room.0.clone()){
						// 获取当前的session_index
						let session_index = Self::get_session_index();

						let mut is_get = false;

						// 超过20个session的算是过期
						for i in 0..20{
							let cur_session = session_index - (i as u32);
							if <InfoOfDisbandRoom<T>>::contains_key(cur_session, room.0.clone()){
								// 奖励本人
								let mut info = <InfoOfDisbandRoom<T>>::get(cur_session, room.0.clone());

								info.already_get_count += 1;

								let reward = info.per_man_reward;
								amount += reward.clone();
								info.already_get_reward += reward;

								<InfoOfDisbandRoom<T>>::insert(cur_session, room.0.clone(), info.clone());

								T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&who, reward));

								// 删除个人
								<ListenersOfRoom<T>>::mutate(room.0.clone(), |h| h.remove(&who));

								// 如果是所有人已经完成 那么就清除
								if info.already_get_count.clone() == info.total_person.clone(){
									<ListenersOfRoom<T>>::remove(room.0.clone());
								}

								is_get = true;
								break;
							}

							// 一般不存在下面的问题
							if cur_session == 0{
								break;
							}
						}
						let mut status = RewardStatus::Expire;
						// 如果已经获取奖励(如果没有获取奖励 那么说明已经过期)
						if is_get {
							status = RewardStatus::Get;
						}

						// 修改状态
						new_rooms.retain(|h| h.0 != room.0.clone());
						new_rooms.push((room.0.clone(), status));

					}

				}
			}

			<AllListeners<T>>::mutate(who.clone(), |h| h.rooms = new_rooms);
			Self::deposit_event(RawEvent::Payout(who.clone(), amount));

			Ok(())
		}


		/// 在群里发红包
		#[weight = 10_000]
		pub fn send_redpacket_in_room(origin, group_id: u64, token: Tokens, lucky_man_number: u32, amount: BalanceOf<T>) -> DispatchResult{
			let who = ensure_signed(origin)?;

			let currency_id = Self::tokens_convert_to_currency_id(token)?;

			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);

			// 金额太小不能发红包
			ensure!(amount >= <BalanceOf<T>>::from(lucky_man_number).checked_mul(&T::RedPacketMinAmount::get()).ok_or(Error::<T>::Overflow)?, Error::<T>::AmountTooLow);

			// 获取红包id
			let redpacket_id = <RedPacketId>::get();

			let redpacket = RedPacket{
				id: redpacket_id,
				currency_id: currency_id,
				boss: who.clone(),
				total: amount.clone(),
				lucky_man_number: lucky_man_number,
				already_get_man: BTreeSet::<T::AccountId>::default(),
				min_amount_of_per_man: T::RedPacketMinAmount::get(),
				already_get_amount: <BalanceOf<T>>::from(0u32),
				end_time: Self::now() + T::RedPackExpire::get(),
			};

			let amount_u128 = amount.saturated_into::<u128>();

			if currency_id == T::GetNativeCurrencyId::get() {
				T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(&who, amount.clone(), WithdrawReasons::TRANSFER.into(), KeepAlive)?);
			}
			else {
				let amount = amount_u128.saturated_into::<MultiBalanceOf<T>>();
				T::MultiCurrency::withdraw(currency_id, &who, amount)?;
			}

			let now_id = redpacket_id.checked_add(1).ok_or(Error::<T>::Overflow)?;
			<RedPacketId>::put(now_id);
			<RedPacketOfRoom<T>>::insert(group_id, redpacket_id, redpacket);

			// 顺便处理过期红包
			Self::remove_redpacket_by_room_id(group_id, false);

			Self::deposit_event(RawEvent::SendRedPocket(group_id, redpacket_id, amount_u128));

			Ok(())

		}


		/// 退群
		#[weight = 10_000]
		fn exit(origin, group_id: u64) {

			let user = ensure_signed(origin)?;

			// 自己要在群里
			ensure!(Self::is_in_room(group_id, user.clone())?, Error::<T>::NotInRoom);

			// 获取群资产
			let mut room = <AllRoom<T>>::get(group_id).unwrap();

			let number = room.now_members_number;

			// 获取群员资产
			let user_amount = room.total_balances - room.group_manager_balances;

			/// 如果退完群里还有人
			if number != 1 {

				let amount = user_amount / room.now_members_number.saturated_into::<BalanceOf<T>>() / 4u32.saturated_into::<BalanceOf<T>>();

				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&user, amount));

				room.now_members_number -= 1;
				room.total_balances -= amount;
				room.group_manager_balances -= amount;

				let mut listeners = <ListenersOfRoom<T>>::get(group_id);

				// 如果退出的是群主 则换群主
				if room.clone().group_manager == user {

					let _ = listeners.take(&room.group_manager);

					let listeners_cp = listeners.clone();

					if let Some(manager) = listeners_cp.clone().iter().last() {

						let new_manager = listeners.take(&manager).unwrap();

						room.group_manager = new_manager.clone();

						listeners.insert(room.group_manager.clone());

					}
				}

				<AllRoom<T>>::insert(group_id, room);

				<ListenersOfRoom<T>>::insert(group_id, listeners);

			}


			// 如果退完没有人 则解散
			else {

				let amount = room.total_balances;

				T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&user, amount));

				let listeners = <ListenersOfRoom<T>>::get(group_id);

				<AllRoom<T>>::remove(group_id);

				<ListenersOfRoom<T>>::remove(group_id);

				// 删除群红包
				Self::remove_redpacket_by_room_id(group_id, true);

				}

			Self::deposit_event(RawEvent::Exit(user, group_id));

		}


		/// 在群里收红包(需要基金会权限 比如基金会指定某个人可以领取多少)
		#[weight = 10_000]
		pub fn get_redpacket_in_room(origin, lucky_man: T::AccountId, group_id: u64, redpacket_id: u128, amount: BalanceOf<T>) {

			let server_id = ensure_signed(origin)?;

			let real_server_id = <ServerId<T>>::get().ok_or(Error::<T>::ServerIdNotExists)?;

			ensure!(server_id.clone() == real_server_id, Error::<T>::NotServerId);

			let who = lucky_man;

			// 自己要在群里
			ensure!(Self::is_in_room(group_id, who.clone())?, Error::<T>::NotInRoom);

			// 红包存在
			ensure!(<RedPacketOfRoom<T>>::contains_key(group_id, redpacket_id), Error::<T>::RedPacketNotExists);

			// 领取的金额足够大
//			ensure!(amount >= T::RedPacketMinAmount::get(), Error::<T>::AmountTooLow);

			let mut redpacket = <RedPacketOfRoom<T>>::get(group_id, redpacket_id).unwrap();

			ensure!(amount >= redpacket.min_amount_of_per_man.clone(), Error::<T>::AmountTooLow);
			// 红包有足够余额
			ensure!(redpacket.total.clone() - redpacket.already_get_amount.clone() >= amount, Error::<T>::AmountNotEnough);
			// 红包领取人数不能超过最大
			ensure!(redpacket.lucky_man_number.clone() > (redpacket.already_get_man.clone().len() as u32), Error::<T>::ToMaxNumber);

			// 一个人只能领取一次
			ensure!(!redpacket.already_get_man.clone().contains(&who), Error::<T>::CountErr);

			// 过期删除数据 把剩余金额给本人
			if redpacket.end_time.clone() < Self::now(){

				Self::remove_redpacket(group_id, redpacket.clone());

				return Err(Error::<T>::Expire)?;
			}

			T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&who, amount.clone()));

			redpacket.already_get_man.insert(who.clone());
			redpacket.already_get_amount += amount.clone();

			// 如果领取红包的人数已经达到上线 那么就把剩余的金额给本人 并删除记录
			if redpacket.already_get_man.clone().len() == (redpacket.lucky_man_number.clone() as usize){
				Self::remove_redpacket(group_id, redpacket.clone());

			}

			if redpacket.already_get_amount.clone() == redpacket.total{
				<RedPacketOfRoom<T>>::remove(group_id, redpacket_id);
			}

			else{
				<RedPacketOfRoom<T>>::insert(group_id, redpacket_id, redpacket);
			}

			// 顺便处理过期红包
			Self::remove_redpacket_by_room_id(group_id, false);

			Self::deposit_event(RawEvent::GetRedPocket(group_id, redpacket_id, amount.clone()));

		}

	}
}


impl <T: Config> Module <T> {


	// 加入群聊的操作
	fn join_do(you: T::AccountId, group_id: u64, inviter: Option<T::AccountId>, payment_type: Option<InvitePaymentType>) -> DispatchResult{

		let room_info = <AllRoom<T>>::get(group_id).unwrap();

		// 获取进群费用
		let join_cost = room_info.join_cost.clone();

		if inviter.is_some() {

			let inviter = inviter.unwrap();
			let payment_type = payment_type.unwrap();

			// 如果需要付费
			if join_cost > <BalanceOf<T>>::from(0u32){
				// 如果是邀请者自己出钱
				if payment_type == InvitePaymentType::inviter{
					// 扣除邀请者的钱(惩罚保留的)
					T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(&inviter, join_cost.clone(), WithdrawReasons::TRANSFER.into(), KeepAlive)?);

					// 以铸币方式给其他账户转账
					Self::pay_for(group_id, join_cost);
				}
				// 如果是进群的人自己交费用
				else{
					T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(&you, join_cost.clone(), WithdrawReasons::TRANSFER.into(), KeepAlive)?);
					Self::pay_for(group_id, join_cost);

				}

			}

			Self::add_info(you.clone(), group_id)

		}

		// 如果自己不是被邀请进来的
		else {
			// 如果需要支付群费用
			if join_cost > <BalanceOf<T>>::from(0u32){
				T::ProposalRejection::on_unbalanced(T::NativeCurrency::withdraw(&you, join_cost.clone(), WithdrawReasons::TRANSFER.into(), KeepAlive)?);
				Self::pay_for(group_id, join_cost);
			}

			Self::add_info(you.clone(), group_id)

			}

		Ok(())
	}


	///  tokens转变成改currency_id
	fn tokens_convert_to_currency_id(token: Tokens) -> Result<CurrencyIdOf<T>, DispatchError> {
		let currency_id: Result<CurrencyId, &'static str> = token.try_into();
		let currency_id= currency_id.map_err(|_| Error::<T>::TokenErr)?;
		Ok(<CurrencyIdOf<T>>::from(currency_id))
	}


	// 支付给其他人
	fn pay_for(group_id: u64, join_cost: BalanceOf<T>){
		let payment_manager_now = Percent::from_percent(5) * join_cost;
		let payment_manager_later = Percent::from_percent(5) * join_cost;
		let payment_room_later = Percent::from_percent(50) * join_cost;
		let payment_treasury = Percent::from_percent(40) * join_cost;
		let mut room_info = <AllRoom<T>>::get(group_id).unwrap();

		// 这些数据u128远远足够 不用特殊处理
		room_info.total_balances += payment_room_later;
		room_info.total_balances += payment_manager_later;
		room_info.group_manager_balances += payment_manager_later;
		room_info.now_members_number += 1u32;
		let group_manager = room_info.group_manager.clone();
		<AllRoom<T>>::insert(group_id, room_info);

		// 给群主
		T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&group_manager, payment_manager_now));

		// 马上给国库
		let teasury_id = Self::treasury_id();
		T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&teasury_id, payment_treasury));

	}


	/// 进群的最后一步 添加数据
	fn add_info(yourself: T::AccountId, group_id: u64){

		// 添加信息
		<ListenersOfRoom<T>>::mutate(group_id, |h| h.insert(yourself.clone()));
		<AllListeners<T>>::mutate(yourself.clone(), |h| h.rooms.push((group_id, RewardStatus::default())));

	}


	/// 获取现在的区块时间
	fn now() -> T::BlockNumber{
		<system::Module<T>>::block_number()
	}



	fn get_session_index() -> SessionIndex{
		0 as SessionIndex
	}


	/// 根据房间号 对过期的红包进行处理
	fn remove_redpacket_by_room_id(room_id: u64, all: bool){
		let redpackets = <RedPacketOfRoom<T>>::iter_prefix(room_id).collect::<Vec<_>>();
		let now = Self::now();

		// 处理所有
		if all{
			for redpacket in redpackets.iter(){
				Self::remove_redpacket(room_id, redpacket.1.clone());
			}
		}

			// 处理过期的红包
		else{
			for redpacket in redpackets.iter(){
				// 如果过期
				if redpacket.1.end_time < now{
					Self::remove_redpacket(room_id, redpacket.1.clone());
				}
		}
		}

	}

	// 判断投票是否结束 (结束 , 通过)
	fn is_vote_end(total_count: u32, vote_info: DisbandVote<BTreeSet<T::AccountId>>, start_time: T::BlockNumber) -> (bool, bool){
		let half = ((total_count + 1) / 2) as usize;
		let total_count = total_count as usize;
		// 如果有票数超过一半
		if vote_info.approve_man.clone().len() >= half || vote_info.reject_man.clone().len() >= half{
			if vote_info.approve_man.clone().len() >= half {
				 (End, Pass)
			}
			else{
				 (End, NotPass)
			}
		}

		else{
			let max = cmp::max(vote_info.approve_man.clone().len(), vote_info.reject_man.clone().len());
			let min = cmp::min(vote_info.approve_man.clone().len(), vote_info.reject_man.clone().len());
			if Percent::from_percent(20) * total_count < max - min{
				if vote_info.approve_man.clone().len() >= vote_info.reject_man.clone().len(){
					 (End, Pass)
				}
				else{
					 (End, NotPass)
				}

			}

			else{
				if start_time + T::VoteExpire::get() >= Self::now(){
					(NotEnd, NotPass)
				}

				// 时间到 结束
				else{
					(End, NotPass)
				}

			}
		}
	}

	pub fn treasury_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}


	/// 判断人是否在群里
	fn is_in_room(group_id: u64, who: T::AccountId) -> result::Result<bool, DispatchError> {
		let _ = <AllRoom<T>>::get(group_id).ok_or(Error::<T>::RoomNotExists)?;
		let listeners = <ListenersOfRoom<T>>::get(group_id);

		if listeners.clone().len() == 0 {
			return Err(Error::<T>::RoomEmpty)?;
		}

		if listeners.contains(&who) {
			Ok(true)
		}
		else {
			Ok(false)
		}
	}


	/// 删除红包
	fn remove_redpacket(room_id: u64, redpacket: RedPacket<T::AccountId, BTreeSet<T::AccountId>, BalanceOf<T>, T::BlockNumber,  CurrencyIdOf<T>>) {
		let who = redpacket.boss.clone();
		let currency_id = redpacket.currency_id.clone();
		let remain = redpacket.total.clone() - redpacket.already_get_amount.clone();
		let redpacket_id = redpacket.id.clone();

		let remain_u128 = remain.saturated_into::<u128>();

		if currency_id == T::GetNativeCurrencyId::get() {
			T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&who, remain));
		}
		else {
			let amount = remain_u128.saturated_into::<MultiBalanceOf<T>>();
			T::MultiCurrency::deposit(currency_id, &who, amount);
		}

		<RedPacketOfRoom<T>>::remove(room_id, redpacket_id);

	}


	/// 排序队列里的account_id
	fn sort_account_id(who: Vec<T::AccountId>) -> result::Result<Vec<T::AccountId>, DispatchError> {

		ensure!(who.clone().len() > 0 as usize, Error::<T>::VecEmpty);

		let mut new_who = vec![];

		let who_cp = who.clone();

		for i in who_cp.iter() {
			// 第一个数直接插入
			if new_who.len() == 0 as usize {
				new_who.insert(0, i.clone());
			}

			else{
				let mut index = 0;

				for j in new_who.iter() {
					if i >= j {
						ensure!(i != j, Error::<T>::MemberDuplicate);
						index += 1;
					}
				}
				new_who.insert(index, i.clone());
			}
		}

		Ok(new_who)
	}


	// 删除过期的解散群产生的信息
	fn remove_expire_disband_info() {
		let session_indexs = <AllSessionIndex>::get();

		let mut session_indexs_cp = session_indexs.clone();

		// 注意  这个执行一次就出来了
		for index in session_indexs.iter(){
			let cur_session_index = Self::get_session_index();
			if cur_session_index - index >= 84u32{
				let mut info = <InfoOfDisbandRoom<T>>::iter_prefix(index).collect::<Vec<_>>();
				// 一次哦删除最多一个session 200条数据
				info.truncate(200);
				for i in info.iter(){
					let group_id = i.0;
					// 删除掉房间剩余记录
					<ListenersOfRoom<T>>::remove(group_id);

					let disband_room_info = <InfoOfDisbandRoom<T>>::get(index, group_id);
					// 获取剩余的没有领取的金额
					let remain_reward = disband_room_info.total_reward - disband_room_info.already_get_reward;

					// 剩余的金额转给国库
					let teasury_id = Self::treasury_id();
					T::Create::on_unbalanced(T::NativeCurrency::deposit_creating(&teasury_id, remain_reward));

					// 每个房间
					<InfoOfDisbandRoom<T>>::remove(index, group_id);

				}

				// 如果已经完全删除 那么把这个index去掉
				let info1 = <InfoOfDisbandRoom<T>>::iter_prefix(index).collect::<Vec<_>>();
				if info1.is_empty(){
					<AllSessionIndex>::put(session_indexs_cp.split_off(1));
				}

			}
			break;

		}

	}

	}



decl_event!(
	pub enum Event<T> where
	 <T as system::Config>::AccountId,
	 Amount = <<T as pallet_transfer::Config>::NativeCurrency as Currency<<T as system::Config>::AccountId>>::Balance,
	 {
		 SetMultisig,
		 AirDroped(AccountId, AccountId),
		 CreatedRoom(AccountId, u64),
		 Invited(AccountId, AccountId),
		 IntoRoom(AccountId, Option<AccountId>, u64),
		 RejectedInvite(AccountId, u64),
		 ChangedPermission(AccountId, u64),
		 BuyProps(AccountId),
		 BuyAudio(AccountId),
		 Kicked(AccountId, u64),
		 AskForDisband(AccountId, u64),
		 DisbandVote(AccountId, u64),
		 Payout(AccountId, Amount),
		 SendRedPocket(u64, u128, u128),
		 GetRedPocket(u64, u128, Amount),
		 JoinCostChanged(u64, Amount),
		 SetPropsPrice,
		 SetAudioPrice,
		 SetDisbandInterval,
		 SetKickInterval,
		 SetCreateCost,
		 SetServerId(AccountId),
		 Exit(AccountId, u64),
		 ManagerGetReward(AccountId, Amount, Amount),
	}
);





