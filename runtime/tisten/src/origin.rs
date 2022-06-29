use super::*;

parameter_types! {
	pub ListenFoundationAccounts: Vec<AccountId> = vec![
		hex_literal::hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into(),	// Alice
		hex_literal::hex!["8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"].into(),	// Bob
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
pub type EnsureRootOrThreeFourthsCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
>;
