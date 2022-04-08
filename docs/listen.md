# listen模块文档
## 重要方法
1. 设置多签账户
    * 代码 `pub fn set_multisig(
            origin: OriginFor<T>,
            members: Vec<<T::Lookup as StaticLookup>::Source>,
            threshould: u16,
        )`
    * 参数
        * `members` 参与多签的账户
        * `threshould` 多签的阀值
    * 逻辑
        * listen基金会账号才能设置
        * 参与人数大于1小于等于20，阀值大于0并且小于人数
        * 参与的人员名单必须是不重复的

2. 领取空投
    * 代码 `pub fn air_drop(
            origin: OriginFor<T>,
            members: Vec<<T::Lookup as StaticLookup>::Source>,
        )`
    * 参数
        * `members` 获取空投奖励的账号名单
    * 逻辑
        * 多签账号去领取
        * members长度不能大于100(最多领取100个人的)
        * members不能有重复的账号
        * 如果members中有人领取过(账户金额不为0)， 那么全部领取失败
        * 领取成功，每个人获取0.99个LT
    > 优化点：不再需要一个已经领取空投的名单，直接判断账户金额是否为0即可，为0直接给币子
3. 创建房间
    * 代码 `pub fn create_room(
            origin: OriginFor<T>,
            max_members: u32,
            group_type: Vec<u8>,
            join_cost: MultiBalanceOf<T>,
            is_private: bool,
        )`
    * 参数
        * `max_members` 群人数上限
        * `group_type` 群的描述信息（类别）
        * `join_cost` 用户加入群组的费用
        * `is_private` 是否私人（不对外开放）
    * 逻辑
        * 根据人数上限对群主进行扣费，并且这笔费用进入国库中
4. 群主领取工资
    * 代码 `pub fn manager_get_reward(origin: OriginFor<T>, group_id: RoomId)`
    * 参数
        * `group_id` 房间id
    * 逻辑
        * 群主才有权限
        *
        * 计算公式
            * 原始奖励金额 = 群累计消费金额 * 周期数.min(5)    //(一天可以领取一次)
            * 群主奖励金额 = 原始奖励金额 * 群主比例(ManagerProportion)
            * 房间奖励金额 = 原始奖励金额 * 房间比例(RoomProportion)
            * 房间更新后的金额 = 原来的总金额 + 房间奖励金额 - 群主奖励金额
    > total_balances里面的金额来源不单单是消费金额，也有可能是领取工资时候有盈余
5. 群主更新进群费用
    * 代码 `pub fn update_join_cost(
            origin: OriginFor<T>,
            group_id: RoomId,
            join_cost: MultiBalanceOf<T>,
        )`
    * 参数
        * `group_id` 房间id
        * `join_cost` 进群费用
    * 逻辑
        * 群主才能操作
        * 群没有在等待解散队列
