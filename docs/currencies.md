# ***currencies说明模块文档***
## 重要数据结构
```buildoutcfg
pub struct ListenAssetMetadata {
	/// 项目名称
	pub name: Vec<u8>,
	/// 代币名称
	pub symbol: Vec<u8>,
	/// 代币精度
	pub decimals: u8,
}
```
```buildoutcfg
pub struct ListenAssetInfo<AccountId, ListenAssetMetadata> {
	/// 发行此代币的人
	pub owner: AccountId,
	/// 此代币的元数据
	pub metadata: Option<ListenAssetMetadata>,
}
```
## 重要存储
* `ListenAssetsInfo` 资产信息
## 重要方法
1. 创建资产
	* 代码
	```buildoutcfg
		pub fn create_asset(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			amount: BalanceOf<T>,
			metadata: Option<ListenAssetMetadata>,
		)
	```
 	* 参数
		* `currency_id` 资产id
		*  `amount` 发行多少
		* `metadata` 资产元数据
	* 逻辑
		* 资产还没有被创建,如果已经创建，则不能再次创建
2. 设置资产元数据
	* 代码
	```buildoutcfg
		pub fn set_metadata(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			metadata: ListenAssetMetadata,
		)
	```
 	* 参数
		* `currency_id` 资产id
		* `metadata` 资产元数据
	* 逻辑
		* 项目名称大于2个字节，代币名称大于1个字节，精度大于0小于19
		* 必须是资产发行方才能修改
		* 精度不能修改
3. 转账
	* 代码
	```buildoutcfg
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyId,
			#[pallet::compact] amount: BalanceOf<T>,
		)
	```
 	* 参数
		* `dest` 接收方
		* `currency_id` 资产id
		* `amount` 转账金额
	* 逻辑
		* 该资产有元数据才能转账
4. 销毁自己的资产
	* 代码
	```buildoutcfg
		pub fn burn(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			amount: BalanceOf<T>,
		)
	```
 	* 参数
		* `currency_id` 资产id
		* `amount` 销毁金额
	* 逻辑
		* 该资产必须有元数据，才能进行销毁
* root权限重新设置某个账户的金额
	* 代码
	```buildoutcfg
		pub fn update_balance(
			origin: OriginFor<T>,
			who: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyId,
			amount: AmountOf<T>,
		)
	```
 	* 参数
	  	* `who` 目标账户
	  	* `currency_id` 资产id
	  	* `amount` 新的自由金额
	* 逻辑
		* sudo或者公投账户才能操作
		* 现在的金额设为账户的新余额
