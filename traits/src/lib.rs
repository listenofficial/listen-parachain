#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::{prelude::*, result};

pub trait ListenHandler<RoomIndex, AccountId, DispatchErr, Balance> {
	fn get_room_council(room_id: RoomIndex) -> result::Result<Vec<AccountId>, DispatchErr>;
	fn get_prime(room_id: RoomIndex) -> result::Result<Option<AccountId>, DispatchErr>;
	fn get_root(room_id: RoomIndex) -> result::Result<AccountId, DispatchErr>;
	fn get_room_free_amount(room_id: RoomIndex) -> Balance;
	fn sub_room_free_amount(room_id: RoomIndex, amount: Balance)
		-> result::Result<(), DispatchErr>;
}

pub trait CollectiveHandler<RoomIndex, BlockNumber, DispatchErr> {
	fn remove_room_collective_info(room_id: RoomIndex) -> result::Result<(), DispatchErr>;
	fn get_motion_duration(room_id: RoomIndex) -> BlockNumber;
}

pub trait RoomTreasuryHandler<RoomIndex> {
	fn remove_room_treasury_info(room_id: RoomIndex);
}
