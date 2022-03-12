# ***NFT模块说明文档***
## 基本数据结构
```buildoutcfg
pub struct ClassInfo<TokenId, AccountId, Data, ClassMetadataOf> {
	/// 该class的链下信息
	pub metadata: ClassMetadataOf,
	/// 该class中的nft的发行量
	pub total_issuance: TokenId,
	/// 该class的发行人
	pub issuer: AccountId,
	/// 该class的基本信息
	pub data: Data,
}
```
```buildoutcfg
pub struct ClassData<NftLevel, Balance, TokenId> {
	/// 该类型nft的级别（也是类名称）
	level: NftLevel,
	/// 领取该类型下的nft需要消耗多少LTP
	like_threshold: Balance,
	/// 领取该类型下的nft需要消耗多少LT
	claim_cost: Balance,
	/// 该class的大图片hash（默认没有）
	images_hash: Option<Vec<u8>>,
	/// 该class中的nft允许发行的最大数量
	maximum_quantity: TokenId,
}
```
> 以上是关于class的数据类型
***
```buildoutcfg
pub struct TokenInfo<AccountId, Data, TokenMetadataOf> {
	/// Token metadata
	pub metadata: TokenMetadataOf,
	/// nft的所有者
	pub owner: Option<AccountId>,
	/// nft的其他数据
	pub data: Data,
}
```
```buildoutcfg
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct TokenData<Hash, AccountId, Attribute, Balance, NftStatus, ClassId> {
	/// 该nft所属的class
	class_id: ClassId,
	/// 该nft的hash
	hash: Hash,
	/// 领取该nft需要消耗的LTP的数量
	like_threshold: Balance,
	/// 领取该nft需要消耗的LT数量
	claim_cost: Balance,
	/// 该nft的属性
	attribute: Attribute,
	/// 该nft的图片hash
	image_hash: Vec<u8>,
	/// 该nft的出售记录
	sell_records: Vec<(AccountId, Balance)>,
	/// 该nft现在的状态
	status: NftStatus,
}
```
> 以上是nft的数据结构
```buildoutcfg
// nft level(这个将被取消 不用关注)
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum NftLevel {
	Rookie,
	Angle,
	WallStreetElite,
	UnicornHunter,
	Mafia,
	GrandMaster,
	Other(Vec<u8>),
}
```

```buildoutcfg
/// 挂单信息
pub struct SaleInfo<TokenId, Balance, BlockNumber, AccountId> {
	/// 出售人
	seller: AccountId,
	/// 出售的nft
	token_id: TokenId,
	/// 出售的价格
	price: Balance,
	/// 出售的时间
	start_block: BlockNumber,
}
```
```buildoutcfg
/// nft的状态
pub struct NftStatus {
	/// 是否在出售
	is_in_sale: bool,
	/// 是否在激活图片
	is_active_image: bool,
	/// 是否已经有人领取
	is_claimed: bool,
}
```
***
## 重要存储
* `NextClassId` 下一个创建的class id
* `NextTokenId` 下一个创建的nft id
* `InSaleTokens` 在挂单的nft
* `Classes` class的信息
* `Tokens` nft的信息
* `IssuerOf` (不需要关注， 将被删除)
* `TokensOf` 个人的所有nft
* `NoOwnerTokensO` 未被领取的nft
* `AllTokensHash` 所有nft的hash（避免重复创建)
***
## 重要方法
1. 创建一个类别
	* 代码
   ```pub fn create_class(
			origin: OriginFor<T>,
			metadata: Vec<u8>,
			data: ClassDataOf<T>,
		)
   ```
   * 参数:
		* `metadata` 该类链下信息
		* `data` 该类链上基本信息
	* 逻辑:
		* 该类的nftlevel（也就是别名）不能重复
		* 基本数据字节不能超过最大限制

2. 铸造nft
	* 代码
	```buildoutcfg
		pub fn mint(
			origin: OriginFor<T>,
			class_id: T::ClassId,
			metadata: Vec<u8>,
			attribute: Vec<u8>,
			image_hash: Vec<u8>,
		)
	```
	 * 参数
		* `class_id` 在哪个class下创建
		* `metadata` 该nft的链下信息
		* `attribute` 该nft的属性
		* `image_hash` 该nft的图片hash
	* 逻辑
		* 基本数据不能字节过长
		* 该class存在， 并且铸造者是class所有者
		* 不能超出该class允许的最大发行量（每次铸造nft个数加1)
		* 铸造完的币子放入未认领队列（不属于任何人)
3. 领取nft
   * 代码
	```buildoutcfg
		pub fn claim(origin: OriginFor<T>, token: (T::ClassId, T::TokenId))
	```
 	* 参数
		* `token` 领取的nft
	* 逻辑
		* 该nft存在，并且不属于任何人
		* 如果是全网第一次领取该nft，那么领取需要消耗的LT一半给铸造者，一半销毁;其他情况全部销毁
		* 销毁LTP
4. 转账
	* 代码
	```buildoutcfg
		pub fn transfer(
			origin: OriginFor<T>,
			to: T::AccountId,
			token: (T::ClassId, T::TokenId),
		)
	```
 	* 参数
		* `to` 转给谁
		* `token` 转哪个nft
	* 逻辑
		* 该nft存在
		* 该nft不能是在售卖，并且一定是已经认领了的
		* 不能是正在激活状态的nft
		* 转账人必须是该nft所有者
		* 转让所有权
5. 销毁自己的nft
	* 代码
	```buildoutcfg
		pub fn burn(origin: OriginFor<T>, token: (T::ClassId, T::TokenId))
	```
 	* 参数
		* `token` 销毁的token id
	* 逻辑
		* 自己是所有者
		* 该nft不能在售卖u队列
		* 该nft不能是激活状态
		* 归还上一次领取时候需要销毁的LTP给当前的所有者
		* 把该nft放入当前未领取队列
6. 挂单nft(出售)
	* 代码
	```buildoutcfg
		pub fn offer_token_for_sale(
			origin: OriginFor<T>,
			token: (T::ClassId, T::TokenId),
			price: BalanceOf<T>,
		)
	```
 	* 参数
		* `token` 将出售的nft id
		* `price` 以多少价格挂单
	* 逻辑
		* 自己是所有者
		* nft不能是激活状态
		* 还没有在售卖队列
		* 执行成功,该nft进入售卖队列
7. 取消挂单
	* 代码
	```buildoutcfg
		pub fn withdraw_sale(
			origin: OriginFor<T>,
			token: (T::ClassId, T::TokenId),
		)
	```
 	* 参数
		* `token` nft id
	* 逻辑
		* 自己是所有者
		* 该nft正在售卖队列
8. 买nft
	* 代码
	```buildoutcfg
		pub fn buy_token(origin: OriginFor<T>, token: (T::ClassId, T::TokenId))
	```
 	* 参数
		* `token` nft id
	* 逻辑
		* 在售卖队列
		* 转挂单价格对应的LT给所有者
		* 执行成功，转移所有权

9. 激活nft
	* 代码
	```buildoutcfg
		pub fn active(origin: OriginFor<T>, token: (T::ClassId, T::TokenId))
	```
 	* 参数
		* `token` nft id
	* 逻辑
		* 自己是所有者
		* 该nft不能在售卖队列
		* 还没有激活
		* 如果有多个nft，最多只能有一个处于激活状态
10. 关闭激活状态
	* 代码
	```buildoutcfg
		pub fn inactive(origin: OriginFor<T>, token: (T::ClassId, T::TokenId))
	```
 	* 参数
		`token` nft id
	* 逻辑
		* 自己是所有者

