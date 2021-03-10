# listen链上功能实现
## 说明
* 所有额外添加的功能集中在一个模块中， 模块名称是Listen
* 除了账户金额， 所有记录均是用完即销毁，或是定期销毁，没有永久存储（个人购买道具与语音部分目前还是永久，后面会有计划删除)
* 额外添加的多资产模块， 后期使用，目前跟链下功能无关联
* 房间id是自增的u64类型
***
## 主要方法
1. 空投
	* 代码: `fn air_drop(origin, des: T::AccountId)`
	* 参数：
		- des: 空投的目标地址
	* 逻辑:
		- 需要多签权限
		- 一个账户只能被空投一次
		- 国库给空投账户转账0.99个LT
		- 空投的账户不能被删除(不会有灰尘处理)
	>>> 要空投必须先创建一个多签地址， 然后用那个多签地址进行空投
***
2. 创建房间
	* 代码： `fn create_room(origin, max_members: GroupMaxMembers, group_type: Vec<u8>, join_cost: BalanceOf<T>)`
	* 参数:
		- max_members： 房间人数上限(不是自定义)
		- group_type： 房间类型(自己输入字符串，自定义)
		- join_cost: 其他人加入房间需要花费的金额
	* 逻辑
		- 任何人都可以创建房间
		- 根据房间人数上限收取创建费用(账上余额不够，不给创建)， 并且费用直接转到国库
	>>> 创建费用收取： 10人群，1LT； 100人群， 10LT； 500人群， 30LT； 10000人群， 200LT； 不限制， 1000LT
***
3. 创建一个多签的账号(用于空投)
	* 代码： `fn set_multisig(origin, who: Vec<T::AccountId>, threshould: u16)`
	* 参数：
		- who： 参与多签的所有人员id
		- threshould： 阀值(有多少个人签名就可以执行空投操作)
	* 逻辑：
		- root权限(或是公投)
	>>> 一定要先有多签账号，才能进行空投

***
4. 进群
	* 代码： `fn into_room(origin, group_id: u64, invite: T::AccountId, inviter: Option<T::AccountId>, payment_type: Option<InvitePaymentType>)`
	* 参数：
		- group_id： 房间号
		- invite: 被邀请人
		- inviter: 邀请人
		- payment_type: 付费类型(邀请人付费或是被邀请人付费)(可以为空值)
		>>> 说明： 如果有邀请人，邀请人不能是自己，付费类型也不能为空；没有邀请人，说明是自己进群。
	* 逻辑：
		- 签名
		- 房间存在
		- 如果群总人数已经达到上限，不能进群
		- 如果已经在群里，不能再次进入
		- 从付费人的账号余额里，扣除进群费用，费用不够，不给进群

	>>> 进群费用明细： 5%直接转到群主账上； 5%先统计在群里， 解散后给群主； 50%直接转给国库； 40%统计到群里，作为公共费用，群解散后均分
***
5. 群主修改进群费用
	* 代码：`fn update_join_cost(origin, group_id: u64, join_cost: BalanceOf<T>)`
	* 参数：
		- group_id： 房间id
		- join_cost: 费用
	* 逻辑：
		- 房间存在，并且是群主
		- 金额不能跟之前的一样
***
6. 在群里购买道具
	* 代码： `fn buy_props_in_room(origin, group_id: u64, props: AllProps)`
	* 参数：
		- group_id: 房间号
		- props： 道具类型以及分别需要买多少
	* 逻辑：
		- 签名
		- 群存在， 并且自己在群里
		- 根据道具类型与数量计算费用
		- 扣除费用(费用不够，不给操作)， 并且统计到群里， 等解散后均分
		- 购买详情个人记录有一份
	>>> 道具类型有： 文字、图片、视频(具体费用收取看白皮书)
***
7. 在群里购买语音
	* 代码： `fn buy_audio_in_room(origin, group_id: u64, audio: Audio)`
	* 参数：
		- group_id：房间id
		- audio：语音类型与数量
	* 逻辑：
		- 签名
		- 群存在， 并且自己在群里
		- 根据语音类型与数量计算费用
		- 扣除费用(费用不够，不给操作)， 并且统计到群里， 等解散后均分
		- 购买详情个人记录有一份
	>>> 语音类型有： 10s, 30s, 60s, 具体收费情况看白皮书

***

8. 群主踢人
	* 代码： `fn remove_someone(origin, group_id: u64, who: T::AccountId)`
	* 参数：
		- group_id： 房间id
		- who： 即将被踢出群聊的人
	* 逻辑：
		- 群存在
		- 必须是群主才能踢人
		- 被踢的人在群里
		- 群解散后， 这个被踢的人不被奖励（这里逻辑要重新审核）
		- 群主不能随意踢人： 一次仅可以踢一个人； 踢人有时间间隔要求(在可以踢人的时间段才能踢)， 具体看白皮书
***
9. 要求解散群
	* 代码： `fn ask_for_disband_room(origin, group_id: u64)`
	* 参数：
		- 房间id： group_id
	* 逻辑：
		- 群存在， 并且要求解散的人是群里成员
		- 该群还未处于投票状态
		- 自己申请， 算自己一个赞成票
		- 解散群提案有时间间隔要求（从本群上一次解散提按结束时间算起， 具体参看白皮书）
		- 需要花费进群费用的1/10， 并且直接转到国库
		- 议案3天过期
***
10. 给解散群的提案进行投票
	* 代码： `fn vote(origin, group_id: u64, vote: VoteType)`
	* 参数：
		- 房间id： group_id
		- vote： 投票类型(赞成或是反对)
	* 逻辑：
		- 群存在，并且投票人是群成员
		- 群必须是正在投票阶段
		- 不能投两次相同的票(可以换投反对票)
		- 如果提按结束并且通过： 把所有剩余的红包余额归还发红包的人； 把属于群主个人独享的部分奖励给群主（剩下的公共部分需要个人自己认领)
		- 如果提案结束未通过， 把投票记录删除
***
11. 自己领取奖励(一键领取)
	* 代码: `fn pay_out(origin)`
	* 参数无
	* 逻辑:
		- 之前有加入的房间
		- 有房间奖励处于未领取状态
		- 奖励只能领取最多20个session的， 其余算过期处理(过期的数据被清除)
	>>> 注意： 这个操作是领取本人所有能够领取的奖励
***
12. 在群里发红包
	* 代码： `pub fn send_redpacket_in_room(origin, group_id: u64, lucky_man_number: u32, amount: BalanceOf<T>)`
	* 参数：
		- group_id： 房间id
		- lucky_man_number： 打算发给多少人
		- amount： 红包总金额
	* 逻辑：
		- 群存在，并且自己在群里
		- 红包金额有最小限制， 小余则不能发红包(最小金额 = 每个人领取的最小金额(系统定) * lucky_man_number)
		- 从自己账上扣除金额
		- 红包过期时间是1天
		- 顺便处理房间所有过期红包
***
13. 在群里收红包
	* 代码： `pub fn get_redpacket_in_room(origin, lucky_man: T::AccountId, group_id: u64, redpacket_id: u128, amount: BalanceOf<T>)`
	* 参数：
		- lucky_man： 领给谁
		- group_id： 房间id
		- redpacket_id： 红包id
		- amount： 领取金额
	* 逻辑：
		- 需要服务器权限（特定账号)
		- 群存在， 自己在群里，并且这个红包存在
		- （这里有个逻辑值得商榷: 是否要限制领取红包最小金额)
		- 一个人只能领一次
		- 红包如果已经过期， 过期则把剩余的金额归还发红包的人
		- 如果领取红包的人数已经达到上限， 剩余金额归还给发红包的人
		- 顺便处理群里所有过期红包
***
14. 设置语音价格
    * 代码: `fn set_audio_price(origin, cost: AudioPrice<BalanceOf<T>>)`
    * 参数: cost(一个结构体，包含每种语音每条收费多少)
    * 逻辑： root权限
***
15. 设置道具价格
    * 代码: `fn set_props_price(origin, cost: PropsPrice<BalanceOf<T>>)`
    * 参数: cost(一个结构体， 包含每种道具的价格)
    * 逻辑： root权限
***
16. 设置群主踢人的时间间隔
    * 代码: `fn set_remove_interval(origin, time: KickTime<T::BlockNumber>)`
    * 参数: time(一个结构体， 包含每种房间类型多长间隔)
    * 逻辑: root权限
***
17. 设置解散群提案的时间间隔
    * 代码: `fn set_disband_interval(origin, time: DisbandTime<T::BlockNumber>)`
    * 参数： time(一个结构体， 包含每种房间类型多长间隔)
    * 逻辑： root权限
***
18. 设置创建群需要的费用
    * 代码： `fn set_create_cost(origin, max_members: GroupMaxMembers, amount: Balance) `
    * 参数：
        * max_members： 群上限人数（也是群类型)
        * amount: 金额
    * 逻辑：
        * 公投
19. 设置服务器的id（用来领取红包)
    * 代码: `fn set_server_id(origin, account_id: T::AccountId)`
    * 参数:
        * account_id: 服务器id
    * 逻辑:
        * 多签权限来执行
        * `<ServerId<T>>::put(account_id.clone())`
20. 自动退群
	* 代码： `fn exit(origin, group_id: u64)`
	* 参数:
		* group_id: 房间id
	* 逻辑
		* 群存在并且自己在群里

		* 如果退完群里还有人
			* 分得（群总资产 - 群主那份） / 总人数 / 4 资产
			* 如果自己是群主， 那么就选出排名第二的作为群主
		* 如果退出完没有人，则分全部群资产， 有红包的话退回所有红包剩余金额给发红包的人
***
## 主要数据结构
```bazaar
/// 所有道具的统计
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct AllProps{
	picture: u32, // 图片有多少
	text: u32, // 文字有多少
	video: u32, // 视频有多少
}


```
***
```bazaar
/// 语音时长类型的统计
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct Audio{
	ten_seconds: u32,  // 10秒有多少
	thirty_seconds: u32,  // 30秒有多少
	minutes: u32,  // 一分钟有多少
}

```
***
```bazaar
/// 群的奖励信息
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct RoomRewardInfo<Balance>{
	total_person: u32,  // 一共奖励的人数
	already_get_count: u32, // 已经奖励的人数
	total_reward: Balance,  // 总奖励
	already_get_reward: Balance, // 已经领取的奖励
	per_man_reward: Balance,  // 平均每个人的奖励

}
```
***
```bazaar
/// 解散投票
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct DisbandVote<BTreeSet>{
	approve_man: BTreeSet, // 同意解散的所有人
	reject_man: BTreeSet, // 拒绝解散的所有人
}
```
***
```bazaar
/// 红包
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct RedPacket<AccountId, BTreeSet, Balance, BlockNumber>{

	id: u128, // 红包id
	boss: AccountId,  // 发红包的人
	total: Balance,	// 红包总金额
	lucky_man_number: u32, // 红包奖励的人数
	already_get_man: BTreeSet, // 已经领取红包的人
	min_amount_of_per_man: Balance, // 每个人领取的最小红包金额
	already_get_amount: Balance, // 已经总共领取的金额数
	end_time: BlockNumber, // 红包结束的时间

}
```
***
```bazaar
/// 群的类型
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum GroupMaxMembers{
	Ten,  // 10人群
	Hundred, // 100人群
	FiveHundred, // 500人群
	TenThousand, // 1000010人群
	NoLimit,  // 不作限制
}

```
***
```bazaar
/// 投票类型
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum VoteType{
	Approve, // 投同意
	Reject,  // 投反对
}
```
***
```bazaar
// 个人在某房间的领取奖励的状态
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum RewardStatus{
	Get, // 已经领取
	NotGet,  // 还没有领取
	Expire, // 过期

}
```
***
```bazaar
/// 邀请第三人进群的缴费方式
#[derive(PartialEq, Encode, Decode, RuntimeDebug, Clone)]
pub enum InvitePaymentType{
	inviter,  // 邀请人交费
	invitee,  // 被邀请人自己交
}
```
***
```bazaar
/// 群的信息
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct GroupInfo<AccountId, Balance, AllProps, Audio, BlockNumber, GroupMaxMembers, DisbandVote, Moment>{
	group_id: u64,  // 群的id直接用自增的u64类型

	create_payment: Balance,  // 创建群时支付的费用

	group_manager: AccountId,  // 群主
	max_members: GroupMaxMembers, // 最大群人数

	group_type: Vec<u8>, // 群的类型（玩家自定义字符串）
	join_cost: Balance,  // 这个是累加的的还是啥？？？

	props: AllProps,  // 本群语音购买统计
	audio: Audio, // 本群道具购买统计

	total_balances: Balance, // 群总币余额
	group_manager_balances: Balance, // 群主币余额

	now_members_number: u32, // 目前群人数

	last_remove_height: BlockNumber,  // 群主上次踢人的高度
	last_disband_end_hight: BlockNumber,  // 上次解散群提议结束时的高度

	disband_vote: DisbandVote, // 投票信息
	this_disband_start_time: BlockNumber, // 解散议案开始投票的时间

	is_voting: bool,  // 是否出于投票状态
	create_time: Moment,

}

```
***
```bazaar
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct PersonInfo<AllProps, Audio, Balance, RewardStatus>{
	props: AllProps, // 这个人的道具购买统计
	audio: Audio, // 这个人的语音购买统计
	cost: Balance, // 个人购买道具与语音的总费用
	rooms: Vec<(RoomId, RewardStatus)>,  // 这个人加入的所有房间
}
```
***
```buildoutcfg
/// 群主踢人的时间限制
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct KickTime<BlockNumber>{
	Ten: BlockNumber,  // 上限人数为10时候
	Hundred: BlockNumber,  // 上限人数为100时候
	FiveHundred: BlockNumber,  // 上限人数为500时候
	TenThousand: BlockNumber,  // 上限人数为10000时候
	NoLimit: BlockNumber,  // 不作限制的时候
}
```
***
```buildoutcfg
/// 群解散的时间限制
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct DisbandTime<BlockNumber>{
	Ten: BlockNumber,  // 上限人数为10时候
	Hundred: BlockNumber,  // 上限人数为100时候
	FiveHundred: BlockNumber,  // 上限人数为500时候
	TenThousand: BlockNumber,  // 上限人数为10000时候
	NoLimit: BlockNumber,  // 不作限制的时候
}
```
***
```buildoutcfg
/// 道具的价格
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct PropsPrice<BalanceOf>{
	picture: BalanceOf,  // 图片价格
	text: BalanceOf,  // 文字价格
	video: BalanceOf,  // 视频价格
}
```
***
```buildoutcfg
/// 语音的价格
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct AudioPrice<BalanceOf>{
	ten_seconds: BalanceOf,  // 10秒价格
	thirty_seconds: BalanceOf,  // 30秒价格
	minutes: BalanceOf,  // 60秒价格
}
```
***
