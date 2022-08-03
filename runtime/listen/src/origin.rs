use super::*;

parameter_types! {
	pub ListenFoundationAccounts: Vec<AccountId> = vec![
		hex_literal::hex!["60d22a3d6f8d9d7c5bbb1a5bb4ccaea4cdc465e361d78ef9baaddddbc7823e33"].into(),   // root
		hex_literal::hex!["d8c0376e6207982595036a5857ef96a794716784fcbb736e30f00274d95e5c33"].into(),	// 5GxuJP7KpBBzjAbtV3WzYB8Svb9RrbMmYLxAQTiWbGkp8jyQ
		hex_literal::hex!["e0b3982f4bcc7d18f5bab773efbb3b0e245d65104eaaa28e5ecbc0c545f7bc27"].into(),	// 5H9Kw8MJYNrXpRCNSxqQw8VwwtWvt5pP3wuLkv3mZnFUiWEU
		hex_literal::hex!["a842d55126265db0fd114a4cc854f65e7850129cbbd0ade3b24f23820c158665"].into(),	// 5FsKkmUvb4UBq2RwAFH9b8E35GpbCrAbHRRVgTceeFzYimPo
		hex_literal::hex!["0eb40a404c1010b212e904217261ae0aa852b068298eb02f1a51c60abea6ee2b"].into(),	// 5CPz1Zwv49d6BkkdpQFRp81EfME8Jsmzxe89rbm6JbRskgk1
		hex_literal::hex!["942b48158d635dd0f7924031f5823cb4142d449533df32c0a5330b0842d7fc4e"].into(),	// 5FQyoSCbcnodfunhcC7ZpwKkad8JSFxLaZ54aoZyb7HXoX3h
	];
}

pub struct EnsureListenFoundation;
impl EnsureOrigin<Origin> for EnsureListenFoundation {
	type Success = AccountId;
	fn try_origin(o: Origin) -> Result<Self::Success, Origin> {
		Into::<Result<RawOrigin<AccountId>, Origin>>::into(o).and_then(|o| match o {
			RawOrigin::Signed(caller) =>
				if ListenFoundationAccounts::get().contains(&caller) {
					Ok(caller)
				} else {
					Err(Origin::from(Some(caller)))
				},
			r => Err(Origin::from(r)),
		})
	}
}

// We allow root and the Relay Chain council to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin =
	EnsureOneOf<EnsureRoot<AccountId>, EnsureXcm<IsMajorityOfBody<RelayLocation, ExecutiveBody>>>;

pub type HalfRoomCouncil = pallet_dao::EnsureProportionMoreThan<AccountId, RoomCollective, 1, 2>;
pub type RoomRoot = pallet_dao::EnsureRoomRoot<Runtime, AccountId, RoomCollective>;
pub type RoomRootOrHalfRoomCouncil = EnsureOneOf<RoomRoot, HalfRoomCouncil>;
pub type SomeCouncil = pallet_dao::EnsureMembers<AccountId, RoomCollective, 2>;
pub type HalfRoomCouncilOrSomeRoomCouncil = EnsureOneOf<HalfRoomCouncil, SomeCouncil>;
pub type RoomRootOrHalfRoomCouncilOrSomeRoomCouncil =
	EnsureOneOf<RoomRoot, HalfRoomCouncilOrSomeRoomCouncil>;
pub type EnsureRootOrHalfCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;
// pub type EnsureRootOrThreeFourthsCouncil = EnsureOneOf<
// 	EnsureRoot<AccountId>,
// 	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
// >;
